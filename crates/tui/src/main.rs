mod cover;
mod mpv;
mod tui_app;
mod ui;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use tokio::sync::mpsc;
use tokio::time::timeout;

use bvox_core::CorePaths;
use bvox_core::api::{ApiClient, REFERER, UA, VideoItem, quality_label};
use bvox_core::player::Player as _;
use bvox_core::session::{CoreEvent, Msg, Session, parse_bvid};
use mpv::MpvPlayer;
use tui_app::{Action, TuiState};

/// TUI-only events: keys, resizes and exit signals. Everything session-related
/// flows through `Session::pump` instead.
enum TuiEvent {
    Key(KeyEvent),
    Quit,
    Wake,
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
    let api = ApiClient::new(CorePaths::desktop_default())?;
    println!("搜索: {keyword}");
    let items = api.search(keyword).await?;
    print_items(&items);
    Ok(())
}

async fn headless_favorites(media_id: i64) -> Result<()> {
    let api = ApiClient::new(CorePaths::desktop_default())?;
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
    let api = ApiClient::new(CorePaths::desktop_default())?;
    let detail = api.view(&bvid).await?;
    let preference = api.audio_quality();
    let vip = api
        .user_info()
        .await
        .ok()
        .flatten()
        .is_some_and(|user| user.vip);
    let source = api.audio_source(&bvid, detail.cid, preference, vip).await?;

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
        .arg(format!("--referrer={REFERER}"))
        .arg(format!("--user-agent={UA}"))
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
    let api = Arc::new(ApiClient::new(CorePaths::desktop_default())?);
    let player = MpvPlayer::new();
    let (mut session, mut msg_rx) = Session::new(api, player.clone()).await;
    // TUI-specific key hint; keep the restored-playback status when the queue
    // was recovered so the user can tell the session resumed.
    if session.app.results.is_empty() && session.app.now_playing.is_none() {
        session
            .app
            .set_status("按 / 搜索，f 歌单，b 导入 BV，g 登录", false);
    }

    // Must query stdin before the key reader starts consuming events. Keep the
    // timeout short: unsupported terminals otherwise delay startup by 2s.
    // Default to half-blocks inside tmux, where graphics passthrough often
    // needs extra configuration; set BVOX_COVER=native to opt in.
    let cover_mode = std::env::var("BVOX_COVER").unwrap_or_default();
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

    let (tx, mut rx) = mpsc::unbounded_channel::<TuiEvent>();
    spawn_key_reader(tx.clone());
    spawn_signal_handler(tx);

    let mut terminal = ratatui::try_init().context("初始化终端失败")?;
    let mut tui = TuiState::new(picker);
    let result = tui_loop(&mut terminal, &mut session, &mut msg_rx, &mut tui, &mut rx).await;
    // Keep the last position even when the user quits between periodic saves.
    session.save_playback();
    // Stop mpv before the runtime goes away; kill-on-drop is only the fallback.
    let _ = timeout(Duration::from_secs(2), player.quit()).await;
    ratatui::restore();
    result
}

async fn tui_loop(
    terminal: &mut ratatui::DefaultTerminal,
    session: &mut Session,
    msg_rx: &mut mpsc::UnboundedReceiver<Msg>,
    tui: &mut TuiState,
    rx: &mut mpsc::UnboundedReceiver<TuiEvent>,
) -> Result<()> {
    loop {
        tui.sync_selection(&session.app);
        terminal.draw(|frame| ui::draw(frame, &session.app, tui))?;

        tokio::select! {
            event = rx.recv() => match event {
                Some(TuiEvent::Key(key)) => match tui_app::handle_key(&mut session.app, tui, key) {
                    Action::None => {}
                    Action::Quit => break,
                    Action::Search(keyword) => session.search(keyword),
                    Action::LoadMoreSearch => session.load_more_search(),
                    Action::Play(request) => session.play(request),
                    Action::Pause => session.pause().await,
                    Action::Seek(delta) => session.seek(delta).await,
                    Action::Volume(volume) => session.volume(volume).await,
                    Action::SetQuality(quality) => session.set_audio_quality(quality),
                    Action::SetPlayMode(mode) => session.set_play_mode(mode),
                    Action::Login(seq) => session.login(seq),
                    Action::OpenLibrary => session.show_library().await,
                    Action::OpenQueue => session.show_queue(),
                    Action::OpenPlaylist(folder) => session.open_playlist(folder),
                    Action::CreatePlaylist(title) => session.create_playlist(title),
                    Action::CancelPendingImport => session.cancel_pending_import(),
                    Action::RenamePlaylist { folder_id, title } => {
                        session.rename_playlist(folder_id, title)
                    }
                    Action::DeletePlaylist(folder) => session.delete_playlist(folder.id),
                    Action::SetPlaylistHidden { folder_id, hidden } => {
                        session.set_playlist_hidden(folder_id, hidden)
                    }
                    Action::ToggleShowHidden => session.toggle_show_hidden(),
                    Action::RefreshLyrics => session.refresh_lyrics(),
                    Action::SetCacheMode(mode) => session.set_cache_mode(mode),
                    Action::ClearCache => session.clear_cache(),
                    Action::BeginAddTrack(bvid) => session.begin_add_track(bvid),
                    Action::AddToPlaylist { folder_id, bvid } => {
                        session.add_track_to_playlist(folder_id, bvid)
                    }
                    Action::RemoveFromPlaylist { folder_id, bvid } => {
                        session.remove_track_from_playlist(folder_id, bvid)
                    }
                    Action::ImportBv(raw) => session.import_bv(raw),
                },
                Some(TuiEvent::Quit) => break,
                Some(TuiEvent::Wake) => {}
                None => break,
            },
            msg = msg_rx.recv() => match msg {
                Some(msg) => match session.process(msg) {
                    CoreEvent::StateChanged => {}
                    CoreEvent::CoverReady { key, bytes } => tui.apply_cover(&key, &bytes),
                    CoreEvent::Closed => break,
                },
                None => break,
            },
        }
    }

    Ok(())
}

fn spawn_key_reader(tx: mpsc::UnboundedSender<TuiEvent>) {
    std::thread::spawn(move || {
        loop {
            match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    if tx.send(TuiEvent::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    let _ = tx.send(TuiEvent::Wake);
                }
                Ok(_) => {}
                // stdin closed or the terminal hung up: leave instead of
                // lingering in the background with audio still playing.
                Err(_) => {
                    let _ = tx.send(TuiEvent::Quit);
                    break;
                }
            }
        }
    });
}

fn spawn_signal_handler(tx: mpsc::UnboundedSender<TuiEvent>) {
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
        let _ = tx.send(TuiEvent::Quit);
    });
}

fn print_help() {
    println!(
        "\
bvox — B 站音频播放器（终端界面）

用法:
  bvox                     启动 TUI
  bvox --search 关键词      命令行搜索
  bvox --play BV...        命令行播放（mpv，无 TUI）
  bvox --fav MEDIA_ID      命令行列出公开收藏夹内容
  bvox --probe-cover       查看终端图片协议（kitty/sixel/halfblocks）
  bvox --help              显示帮助

TUI 按键:
  / 或 s   搜索              b      导入 BV 号/链接
  Enter    播放 / 打开歌单   space  暂停/继续
  n / p    下一首/上一首      ← / →  后退/前进 5 秒
  + / -    音量               g      扫码登录
  f        歌单列表/刷新
  a        加入歌单           d      移除歌曲/删除歌单
  n / r    新建/重命名歌单（歌单列表内）
  h / H    隐藏歌单 / 显示隐藏项（歌单列表内）
  m        切换播放模式（顺序/列表循环/单曲循环/随机）
  Esc      歌单列表 ↔ 歌曲列表
  l        歌词视图           r      重新获取播放地址
  c        切换缓存策略（自动/仅小文件/关闭）
  C        清理本地缓存（正在播放的缓存会保留）
  q        退出

播放模式:
  顺序播放：播完列表停止；列表循环：播完回到第一首；
  单曲循环：反复播放当前曲目；随机播放：随机顺序，可回退上一首。
  模式会保存在本地，重启后沿用。

歌单:
  登录后主界面是「我的歌单」（B 站收藏夹）。Enter 打开歌单，
  n 新建、r 改名、d 删除；歌曲列表里 a 加入歌单、d 从歌单移除。
  h 隐藏/恢复歌单，H 临时显示隐藏项；
  隐藏列表按账号保存在本地，只影响本机显示。
  导入 BV 时若已打开歌单，会直接加入该歌单；否则弹出「加入歌单」列表，
  可选择现有收藏夹，或选末尾的「＋ 新建歌单」创建并加入；Esc 则只留在本地列表。

缓存:
  音频、封面、歌词和视频信息缓存在 ~/.cache/bvox
  （遵循 XDG_CACHE_HOME）。BVOX_CACHE_DIR 自定义目录，
  BVOX_CACHE_MB 限制总大小（默认 1024，0 关闭缓存），
  BVOX_CACHE_MODE 设默认策略（auto/small_files/off，可被 c 键临时覆盖）。
  占用空间显示在「正在播放」边框上；按 C 清理（保留
  正在播放的缓存文件）。

依赖:
  mpv（播放与跳转由 mpv 负责）
  BVOX_COOKIE 环境变量可选；登录成功后 Cookie 会保存到
  ~/.config/bvox/cookie
"
    );
}
