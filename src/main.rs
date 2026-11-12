mod api;
mod app;
mod cache;
mod cover;
mod mpv;
mod ui;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use app::{Action, App, BvImport, PlayingInfo, qr_matrix};
use image::DynamicImage;
use mpv::{Mpv, PlayerState};
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;

use api::{
    ApiClient, AudioSource, AudioStream, FavoriteFolder, LyricLine, QrPhase, UserInfo, VideoDetail,
    VideoItem, VideoPage, format_secs, quality_label,
};

enum Msg {
    Key(KeyEvent),
    Quit,
    Wake,
    SearchDone {
        seq: u64,
        result: Result<Vec<VideoItem>, String>,
    },
    Playing(PlayingInfo),
    PlayFailed(String),
    PlayerState(PlayerState),
    LoginQr {
        seq: u64,
        matrix: Vec<Vec<bool>>,
    },
    LoginPhase {
        seq: u64,
        status: String,
    },
    LoginSuccess {
        seq: u64,
        user: Option<UserInfo>,
    },
    LoginFailed {
        seq: u64,
        message: String,
    },
    FoldersLoaded(Result<Vec<FavoriteFolder>, String>),
    FavoritesImported {
        folder_title: String,
        result: Result<Vec<VideoItem>, String>,
    },
    CoverReady {
        key: String,
        result: Result<DynamicImage, String>,
    },
    CacheProgress {
        key: String,
        downloaded: u64,
        total: Option<u64>,
    },
    CacheDone {
        key: String,
        ok: bool,
    },
    LyricsReady {
        key: String,
        result: Result<Vec<LyricLine>, String>,
    },
    UserInfo(Option<UserInfo>),
    BvImported(Result<BvImport, String>),
    SyncPulled {
        auto: bool,
        result: Result<Option<(String, Vec<VideoItem>)>, String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    match args.first().map(String::as_str) {
        Some("--search") => {
            let keyword = args[1..].join(" ");
            if keyword.trim().is_empty() {
                print_help();
                bail!("--search 需要关键词");
            }
            headless_search(&keyword).await
        }
        Some("--play") => {
            let Some(input) = args.get(1) else {
                print_help();
                bail!("--play 需要 BV 号或视频链接");
            };
            headless_play(input).await
        }
        Some("--fav") => {
            let Some(media_id) = args.get(1) else {
                print_help();
                bail!("--fav 需要收藏夹 media_id");
            };
            let media_id = media_id
                .parse::<i64>()
                .context("--fav 的 media_id 必须是数字")?;
            headless_favorites(media_id).await
        }
        Some("--probe-cover") => {
            let options = QueryStdioOptions {
                timeout_ms: 400,
                ..Default::default()
            };
            match Picker::from_query_stdio_with_options(options) {
                Ok(picker) => println!("{:?}", picker.protocol_type()),
                Err(error) => println!("halfblocks (query failed: {error})"),
            }
            Ok(())
        }
        Some("-h" | "--help") => {
            print_help();
            Ok(())
        }
        Some(other) => {
            print_help();
            bail!("未知参数: {other}");
        }
        None => run_tui().await,
    }
}

async fn headless_search(keyword: &str) -> Result<()> {
    let api = ApiClient::new()?;
    println!("搜索: {keyword}");
    let items = api.search(keyword).await?;
    print_items(&items);
    Ok(())
}

async fn headless_favorites(media_id: i64) -> Result<()> {
    let api = ApiClient::new()?;
    println!("收藏夹: {media_id}");
    let items = api.favorite_items(media_id).await?;
    print_items(&items);
    Ok(())
}

fn print_items(items: &[VideoItem]) {
    if items.is_empty() {
        println!("没有可播放的视频");
        return;
    }
    for (index, item) in items.iter().enumerate() {
        println!("{:>2}. {}  [{}]", index + 1, item.title, item.duration);
        println!("    {} · {}", item.bvid, item.author);
    }
}

async fn headless_play(input: &str) -> Result<()> {
    let bvid = parse_bvid(input).context("无法识别 BV 号")?;
    let api = ApiClient::new()?;
    let detail = api.view(&bvid).await?;
    let source = api.audio_source(&bvid, detail.cid).await?;

    println!("{} — {}", detail.title, detail.author);
    if detail.pages.len() > 1 {
        println!("共 {} P，播放 P1", detail.pages.len());
    }
    println!(
        "音质 {} · {} kbps{}（Ctrl-C 停止）",
        quality_label(source.quality_id()),
        source.bandwidth() / 1000,
        if source.is_cached() {
            " · 本地缓存"
        } else {
            ""
        }
    );

    let mut command = tokio::process::Command::new("mpv");
    command
        .arg("--no-video")
        .arg("--force-window=no")
        .arg(format!("--referrer={}", api::REFERER))
        .arg(format!("--user-agent={}", api::UA))
        .arg(source.target());
    mpv::harden_command(&mut command);
    let status = command
        .status()
        .await
        .context("启动 mpv 失败，请先安装 mpv")?;

    if !status.success() {
        bail!("mpv 退出码: {status}");
    }
    Ok(())
}

async fn run_tui() -> Result<()> {
    let api = Arc::new(ApiClient::new()?);
    let mpv = Arc::new(Mutex::new(Mpv::new()));
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    // Must query stdin before the key reader starts consuming events. Keep the
    // timeout short: unsupported terminals otherwise delay startup by 2s.
    // Default to half-blocks inside tmux, where graphics passthrough often
    // needs extra configuration; set BILI_COVER=native to opt in.
    let cover_mode = std::env::var("BILI_COVER").unwrap_or_default();
    let in_tmux = std::env::var_os("TMUX").is_some();
    let use_native = cover_mode == "native" || (cover_mode != "blocks" && !in_tmux);
    let picker = if use_native {
        let options = QueryStdioOptions {
            timeout_ms: 400,
            ..Default::default()
        };
        Picker::from_query_stdio_with_options(options)
            .ok()
            .map(Arc::new)
    } else {
        None
    };

    spawn_key_reader(tx.clone());
    spawn_signal_handler(tx.clone());
    spawn_player_poller(mpv.clone(), tx.clone());

    let mut terminal = ratatui::try_init().context("初始化终端失败")?;
    let mut app = App::new(picker);
    spawn_user_info(&api, tx.clone());
    if api.has_cookie().await {
        spawn_sync_pull(&api, tx.clone(), true);
    }
    let result = tui_loop(&mut terminal, &mut app, &api, &mpv, tx, &mut rx).await;
    // Stop mpv before the runtime goes away; kill-on-drop is only the fallback.
    if let Ok(mut player) = timeout(Duration::from_secs(2), mpv.lock()).await {
        player.quit().await;
    }
    ratatui::restore();
    result
}

async fn tui_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    api: &Arc<ApiClient>,
    mpv: &Arc<Mutex<Mpv>>,
    tx: mpsc::UnboundedSender<Msg>,
    rx: &mut mpsc::UnboundedReceiver<Msg>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        let Some(message) = rx.recv().await else {
            break;
        };

        match message {
            Msg::Key(key) => match app.handle_key(key) {
                Action::None => {}
                Action::Quit => break,
                Action::Search(keyword) => {
                    spawn_search(api, tx.clone(), app.search_seq, keyword);
                }
                Action::Play(request) => {
                    spawn_play(api, mpv, tx.clone(), request.item, request.page_index);
                }
                Action::Pause => {
                    let mut player = mpv.lock().await;
                    if !player.is_started() {
                        app.paused = !app.paused;
                        continue;
                    }
                    if let Err(error) = player.toggle_pause().await {
                        app.paused = !app.paused;
                        app.set_error(error);
                    }
                }
                Action::Seek(delta) => {
                    let mut player = mpv.lock().await;
                    if player.is_started()
                        && let Err(error) = player.seek(delta).await
                    {
                        app.set_error(error);
                    }
                }
                Action::Volume(volume) => {
                    let mut player = mpv.lock().await;
                    if player.is_started()
                        && let Err(error) = player.set_volume(volume).await
                    {
                        app.set_error(error);
                    }
                }
                Action::Login(seq) => {
                    spawn_login(api, tx.clone(), seq);
                }
                Action::Favorites => {
                    if api.has_cookie().await {
                        app.open_favorites();
                        spawn_folders(api, tx.clone());
                    } else {
                        app.set_status("请先按 g 扫码登录", true);
                    }
                }
                Action::ImportFolder(folder) => {
                    spawn_import_folder(api, tx.clone(), folder);
                }
                Action::ImportBv(raw) => {
                    spawn_import_bv(api, tx.clone(), raw);
                }
                Action::SyncPull => {
                    if !api.has_cookie().await {
                        app.set_status("请先按 g 扫码登录", true);
                    } else {
                        app.set_status("正在从云同步加载…", false);
                        spawn_sync_pull(api, tx.clone(), false);
                    }
                }
            },
            Msg::Quit => break,
            Msg::Wake => {}
            Msg::SearchDone { seq, result } => app.on_search_done(seq, result),
            Msg::Playing(info) => {
                let key = format!("{}#{}#{}", info.item.bvid, info.cid, info.page_index);
                let bvid = info.item.bvid.clone();
                let cid = info.cid;
                let pic = info.pic.clone();
                app.on_playing(info);
                spawn_cover(api, tx.clone(), key.clone(), pic);
                spawn_lyrics(api, tx.clone(), key, bvid, cid);
            }
            Msg::PlayFailed(error) => app.on_play_failed(error),
            Msg::PlayerState(state) => app.on_player_state(state),
            Msg::LoginQr { seq, matrix } => app.on_login_qr(seq, matrix),
            Msg::LoginPhase { seq, status } => app.on_login_phase(seq, status),
            Msg::LoginSuccess { seq, user } => {
                if app.on_login_success(seq, user) {
                    if let Some(item) = app.now_playing.clone() {
                        // Subtitles often require login; retry for the current track.
                        spawn_lyrics(
                            api,
                            tx.clone(),
                            app.lyrics_key.clone(),
                            item.bvid,
                            app.current_cid,
                        );
                    }
                    spawn_sync_pull(api, tx.clone(), true);
                }
            }
            Msg::LoginFailed { seq, message } => app.on_login_failed(seq, message),
            Msg::FoldersLoaded(result) => app.on_folders_loaded(result),
            Msg::FavoritesImported {
                folder_title,
                result,
            } => app.on_favorites_imported(folder_title, result),
            Msg::CoverReady { key, result } => app.on_cover_image(key, result),
            Msg::CacheProgress {
                key,
                downloaded,
                total,
            } => app.on_cache_progress(key, downloaded, total),
            Msg::CacheDone { key, ok } => app.on_cache_done(key, ok),
            Msg::LyricsReady { key, result } => app.on_lyrics(key, result),
            Msg::BvImported(result) => app.on_bv_imported(result),
            Msg::SyncPulled { auto, result } => app.on_sync_pulled(auto, result),
            Msg::UserInfo(user) => {
                if app.user.is_none()
                    && let Some(user) = user
                {
                    app.user = Some(user.name);
                }
            }
        }

        // Auto-advance once per track end. The poller reports `eof-reached`;
        // take_eof() clears the flag so this cannot loop on the same track.
        if app.take_eof() && !app.pending_play {
            if let Some(request) = app.step(1) {
                spawn_play(api, mpv, tx.clone(), request.item, request.page_index);
            } else {
                app.has_media = false;
                app.set_status("播放列表结束", false);
            }
        }
    }

    Ok(())
}

fn spawn_key_reader(tx: mpsc::UnboundedSender<Msg>) {
    std::thread::spawn(move || {
        loop {
            match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    if tx.send(Msg::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    let _ = tx.send(Msg::Wake);
                }
                Ok(_) => {}
                // stdin closed or the terminal hung up: leave instead of
                // lingering in the background with audio still playing.
                Err(_) => {
                    let _ = tx.send(Msg::Quit);
                    break;
                }
            }
        }
    });
}

fn spawn_signal_handler(tx: mpsc::UnboundedSender<Msg>) {
    use tokio::signal::unix::{SignalKind, signal};

    let (Ok(mut interrupt), Ok(mut terminate), Ok(mut hangup)) = (
        signal(SignalKind::interrupt()),
        signal(SignalKind::terminate()),
        signal(SignalKind::hangup()),
    ) else {
        return;
    };

    tokio::spawn(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
            _ = hangup.recv() => {}
        }
        let _ = tx.send(Msg::Quit);
    });
}

fn spawn_player_poller(mpv: Arc<Mutex<Mpv>>, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let state = {
                let mut player = mpv.lock().await;
                player.poll_state().await
            };
            match state {
                Ok(Some(state)) => {
                    if tx.send(Msg::PlayerState(state)).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
    });
}

fn spawn_user_info(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>) {
    let api = api.clone();
    tokio::spawn(async move {
        let user = api.user_info().await.ok().flatten();
        let _ = tx.send(Msg::UserInfo(user));
    });
}

fn spawn_search(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, seq: u64, keyword: String) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .search(&keyword)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::SearchDone { seq, result });
    });
}

fn spawn_login(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, seq: u64) {
    let api = api.clone();
    tokio::spawn(async move {
        let qr = match api.qr_generate().await {
            Ok(qr) => qr,
            Err(error) => {
                let _ = tx.send(Msg::LoginFailed {
                    seq,
                    message: format!("{error:#}"),
                });
                return;
            }
        };

        let Some(matrix) = qr_matrix(&qr.url) else {
            let _ = tx.send(Msg::LoginFailed {
                seq,
                message: "二维码生成失败".to_string(),
            });
            return;
        };
        if tx.send(Msg::LoginQr { seq, matrix }).is_err() {
            return;
        }

        let mut last_phase = String::new();
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match api.qr_poll(&qr.key).await {
                Ok(QrPhase::Success) => {
                    let user = api.user_info().await.ok().flatten();
                    let _ = tx.send(Msg::LoginSuccess { seq, user });
                    return;
                }
                Ok(QrPhase::Expired) => {
                    let _ = tx.send(Msg::LoginPhase {
                        seq,
                        status: "二维码已过期，按 r 重新生成".to_string(),
                    });
                    return;
                }
                Ok(QrPhase::Scanned) => {
                    if last_phase != "scanned" {
                        last_phase = "scanned".to_string();
                        let _ = tx.send(Msg::LoginPhase {
                            seq,
                            status: "已扫码，请在手机上确认".to_string(),
                        });
                    }
                }
                Ok(QrPhase::Waiting) => {
                    if last_phase != "waiting" {
                        last_phase = "waiting".to_string();
                        let _ = tx.send(Msg::LoginPhase {
                            seq,
                            status: "等待扫码…".to_string(),
                        });
                    }
                }
                Err(error) => {
                    if last_phase != "error" {
                        last_phase = "error".to_string();
                        let _ = tx.send(Msg::LoginPhase {
                            seq,
                            status: format!("状态查询失败，重试中: {error:#}"),
                        });
                    }
                }
            }
        }

        let _ = tx.send(Msg::LoginPhase {
            seq,
            status: "二维码已过期，按 r 重新生成".to_string(),
        });
    });
}

fn spawn_folders(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .favorite_folders()
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::FoldersLoaded(result));
    });
}

fn spawn_import_bv(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, raw: String) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = import_bv(&api, &raw)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::BvImported(result));
    });
}

async fn import_bv(api: &ApiClient, raw: &str) -> Result<BvImport> {
    let bvid = parse_bvid(raw).context("无法识别 BV 号")?;
    let detail = api.view(&bvid).await?;
    if detail.aid == 0 {
        bail!("视频信息缺少 aid，无法加入云同步");
    }

    let total_duration = detail.pages.iter().map(|page| page.duration).sum::<i64>();
    let duration = if total_duration > 0 {
        total_duration
    } else {
        detail.duration
    };
    let item = VideoItem {
        bvid: detail.bvid.clone(),
        title: detail.title.clone(),
        author: detail.author.clone(),
        duration: format_secs(duration),
        pic: detail.pic.clone(),
    };

    let (sync_message, sync_error) = if api.has_cookie().await {
        match api.add_to_sync(detail.aid).await {
            Ok(folder) => (format!("已同步到云收藏夹「{}」", folder.title), false),
            Err(error) => (format!("云同步失败: {error:#}"), true),
        }
    } else {
        ("未登录，仅加入本地列表".to_string(), false)
    };

    Ok(BvImport {
        item,
        sync_message,
        sync_error,
    })
}

fn spawn_sync_pull(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, auto: bool) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let pulled = api.sync_items(!auto).await?;
            Ok::<_, anyhow::Error>(pulled.map(|(folder, items)| (folder.title, items)))
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::SyncPulled { auto, result });
    });
}

fn spawn_import_folder(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder: FavoriteFolder,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .favorite_items(folder.id)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::FavoritesImported {
            folder_title: folder.title,
            result,
        });
    });
}

fn spawn_cover(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    key: String,
    pic: Option<String>,
) {
    let Some(pic) = pic else {
        return;
    };
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let bytes = api.image_bytes(&pic).await?;
            image::load_from_memory(&bytes).context("封面图片解码失败")
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::CoverReady { key, result });
    });
}

fn spawn_lyrics(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    key: String,
    bvid: String,
    cid: i64,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .lyrics(&bvid, cid)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::LyricsReady { key, result });
    });
}

fn spawn_play(
    api: &Arc<ApiClient>,
    mpv: &Arc<Mutex<Mpv>>,
    tx: mpsc::UnboundedSender<Msg>,
    item: VideoItem,
    page_index: usize,
) {
    let api = api.clone();
    let mpv = mpv.clone();
    tokio::spawn(async move {
        let item_pic = item.pic.clone();
        let result: Result<(VideoDetail, AudioSource, usize, usize, String, i64)> = async {
            let detail = api.view(&item.bvid).await?;
            let pages = if detail.pages.is_empty() {
                vec![VideoPage {
                    cid: detail.cid,
                    duration: detail.duration,
                    part: String::new(),
                }]
            } else {
                detail.pages.clone()
            };
            let index = page_index.min(pages.len().saturating_sub(1));
            let page = pages.get(index).context("页码超出范围")?;

            let source = api.audio_source(&detail.bvid, page.cid).await?;
            let target = source.target();
            let mut player = mpv.lock().await;
            player.load(&target).await?;
            Ok((
                detail,
                source,
                index,
                pages.len(),
                page.part.clone(),
                page.duration,
            ))
        }
        .await;

        match result {
            Ok((detail, source, page_index, page_count, part, page_duration)) => {
                let pic = detail.pic.clone().or(item_pic);
                let cid = detail
                    .pages
                    .get(page_index)
                    .map(|page| page.cid)
                    .unwrap_or(detail.cid);
                let bvid = detail.bvid.clone();
                let cached = source.is_cached();
                let quality = format!(
                    "{} · {} kbps",
                    quality_label(source.quality_id()),
                    source.bandwidth() / 1000
                );
                let playing = VideoItem {
                    bvid: detail.bvid,
                    title: detail.title,
                    author: detail.author,
                    duration: format_secs(page_duration),
                    pic: pic.clone(),
                };
                let key = format!("{bvid}#{cid}#{page_index}");
                let _ = tx.send(Msg::Playing(PlayingInfo {
                    item: playing,
                    quality,
                    page_index,
                    page_count,
                    part,
                    page_duration,
                    cid,
                    pic,
                    cached,
                }));

                // A cache miss streams immediately; write a copy in the
                // background so the next play uses the local file.
                if let AudioSource::Remote(stream) = source {
                    spawn_cache_download(&api, tx.clone(), key, bvid, cid, stream);
                }
            }
            Err(error) => {
                let _ = tx.send(Msg::PlayFailed(format!("{error:#}")));
            }
        }
    });
}

fn spawn_cache_download(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    key: String,
    bvid: String,
    cid: i64,
    stream: AudioStream,
) {
    if !api.cache_enabled() {
        return;
    }
    let inflight = format!("{bvid}:{cid}");
    if !api.cache().begin_download(&inflight) {
        return;
    }

    let api = api.clone();
    tokio::spawn(async move {
        let progress_tx = tx.clone();
        let progress_key = key.clone();
        let result = api
            .download_audio(&bvid, cid, &stream, move |downloaded, total| {
                let _ = progress_tx.send(Msg::CacheProgress {
                    key: progress_key.clone(),
                    downloaded,
                    total,
                });
            })
            .await;
        api.cache().end_download(&inflight);
        let _ = tx.send(Msg::CacheDone {
            key,
            ok: result.is_ok(),
        });
    });
}

fn parse_bvid(input: &str) -> Option<String> {
    let start = input.find("BV")?;
    let candidate = &input[start..];
    if candidate.len() < 12 {
        return None;
    }
    let bvid = &candidate[..12];
    if bvid
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        Some(bvid.to_string())
    } else {
        None
    }
}

fn print_help() {
    println!(
        "\
bili-music — B 站音频 TUI 播放器

用法:
  bili-music                    启动 TUI
  bili-music --search 关键词     命令行搜索
  bili-music --play BV...       命令行播放（mpv，无 TUI）
  bili-music --fav MEDIA_ID     命令行列出公开收藏夹内容
  bili-music --probe-cover      查看终端图片协议（kitty/sixel/halfblocks）
  bili-music --help             显示帮助

TUI 按键:
  / 或 s   搜索          b      导入 BV 号/链接
  Enter    播放选中项     space  暂停/继续
  n / p    下一首/上一首  ← / →  后退/前进 5 秒
  + / -    音量           g      扫码登录
  f        收藏夹导入     y      从云同步收藏夹载入
  l        歌词视图       r      重新获取播放地址
  q        退出

云同步:
  登录后，用 b 导入的 BV 会加入私有收藏夹「bili-music 云同步」。
  y 拉取该收藏夹；也可以设置 BILI_SYNC_FOLDER 自定义收藏夹名。

缓存:
  音频、封面、歌词和视频信息缓存在 ~/.cache/bili-music
  （遵循 XDG_CACHE_HOME）。BILI_CACHE_DIR 自定义目录，
  BILI_CACHE_MB 限制总大小（默认 1024，0 关闭缓存）。

依赖:
  mpv（播放与跳转由 mpv 负责）
  BILI_COOKIE 环境变量可选；登录成功后 Cookie 会保存到
  ~/.config/bili-music/cookie
"
    );
}
