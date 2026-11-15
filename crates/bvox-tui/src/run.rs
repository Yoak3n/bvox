//! TUI runtime: terminal setup, event loop and signal handling.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::mpv::MpvPlayer;
use crate::tui_app::{Action, TuiState};
use bvox_core::CorePaths;
use bvox_core::api::ApiClient;
use bvox_core::player::Player as _;
use bvox_core::session::{CoreEvent, Msg, Session};

/// TUI-only events: keys, resizes and exit signals. Everything session-related
/// flows through `Session::pump` instead.
enum TuiEvent {
    Key(KeyEvent),
    Quit,
    Wake,
}
pub(crate) async fn run_tui() -> Result<()> {
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
        terminal.draw(|frame| crate::ui::draw(frame, &session.app, tui))?;

        tokio::select! {
            event = rx.recv() => match event {
                Some(TuiEvent::Key(key)) => match crate::tui_app::handle_key(&mut session.app, tui, key) {
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
                    Action::ImportBv(raw) => session.import_bv(raw, true),
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
