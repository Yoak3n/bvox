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

use bili_core::CorePaths;
use bili_core::api::{ApiClient, REFERER, UA, VideoItem, quality_label};
use bili_core::player::Player as _;
use bili_core::session::{CoreEvent, Msg, Session, parse_bvid};
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
    // TUI-specific key hint; the core default is platform-neutral.
    session
        .app
        .set_status("按 / 搜索，b 导入 BV，g 登录，f 收藏夹，y 云同步", false);

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

    let (tx, mut rx) = mpsc::unbounded_channel::<TuiEvent>();
    spawn_key_reader(tx.clone());
    spawn_signal_handler(tx);

    let mut terminal = ratatui::try_init().context("初始化终端失败")?;
    let mut tui = TuiState::new(picker);
    let result = tui_loop(&mut terminal, &mut session, &mut msg_rx, &mut tui, &mut rx).await;
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
                    Action::Play(request) => session.play(request),
                    Action::Pause => session.pause().await,
                    Action::Seek(delta) => session.seek(delta).await,
                    Action::Volume(volume) => session.volume(volume).await,
                    Action::Login(seq) => session.login(seq),
                    Action::Favorites => session.favorites_open().await,
                    Action::ImportFolder(folder) => session.import_folder(folder),
                    Action::ImportBv(raw) => session.import_bv(raw),
                    Action::SyncPull => session.sync_pull(false).await,
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
