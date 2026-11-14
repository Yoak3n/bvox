//! The session loop: owns the app state, dispatches background-task results
//! into it and drives auto-advance.
//!
//! Frontends feed it user commands (search/play/pause/...) and pump events
//! out. Every state mutation goes through `App::on_*` exactly like the
//! original TUI loop did, so both frontends observe identical behaviour.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc;

use crate::api::{
    ApiClient, AudioQuality, AudioSource, AudioStream, FavoriteFolder, PlayMode, QrPhase,
    SearchPage, VideoDetail, VideoItem, VideoPage, clean_title, cover_thumbnail_url, format_secs,
    quality_label,
};
use crate::app::{
    App, BvImport, MainView, PageSummary, PendingImport, PlayRequest, PlayingInfo, qr_matrix,
};
use crate::cache::CacheMode;
use crate::player::{Player, PlayerConfig};
use crate::resume::SavedPlayback;

/// How far playback may advance before the position is written to disk again.
const PROGRESS_SAVE_INTERVAL_SECS: f64 = 5.0;

/// Internal message vocabulary. `Key`/`Quit`/`Wake` stayed frontend-side;
/// everything else is session business.
pub enum Msg {
    /// Nudge the frontend to re-read state (no other effect).
    StateChanged,
    SearchDone {
        seq: u64,
        keyword: String,
        result: Result<SearchPage, String>,
    },
    SearchMoreDone {
        seq: u64,
        result: Result<SearchPage, String>,
    },
    Playing(PlayingInfo),
    PlayFailed(String),
    PlayerState(crate::player::PlayerState),
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
        user: Option<crate::api::UserInfo>,
    },
    LoginFailed {
        seq: u64,
        message: String,
    },
    PlaylistsLoaded {
        seq: u64,
        result: Result<(Vec<FavoriteFolder>, Vec<i64>), String>,
    },
    PlaylistOpened {
        seq: u64,
        folder: FavoriteFolder,
        result: Result<Vec<VideoItem>, String>,
        /// True when the contents came from the offline cache.
        offline: bool,
    },
    PlaylistHidden {
        folder_id: i64,
        hidden: bool,
        result: Result<(), String>,
    },
    PlaylistCreated {
        result: Result<FavoriteFolder, String>,
        /// Track waiting for this playlist; `None` for a manual create.
        pending: Option<PendingImport>,
    },
    PlaylistRenamed {
        folder_id: i64,
        title: String,
        result: Result<(), String>,
    },
    PlaylistDeleted {
        folder_id: i64,
        result: Result<(), String>,
    },
    TrackAdded {
        folder_id: i64,
        changed: bool,
        result: Result<(), String>,
    },
    TrackRemoved {
        folder_id: i64,
        bvid: String,
        changed: bool,
        result: Result<(), String>,
    },
    CoverReady {
        key: String,
        result: Result<Vec<u8>, String>,
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
        seq: u64,
        key: String,
        result: Result<Vec<crate::api::LyricLine>, String>,
    },
    UserInfo(Option<crate::api::UserInfo>),
    BvImported(Result<BvImport, String>),
}

/// Events a frontend observes from [`Session::pump`].
pub enum CoreEvent {
    /// State changed; re-read the app snapshot / redraw.
    StateChanged,
    /// Cover bytes arrived; `key` is `bvid#cid#page` of the track it belongs to.
    CoverReady { key: String, bytes: Vec<u8> },
    /// The session channel closed; the frontend should exit.
    Closed,
}

pub struct Session {
    pub api: Arc<ApiClient>,
    pub player: Arc<dyn Player>,
    pub app: App,
    tx: mpsc::UnboundedSender<Msg>,
    /// Bumped for every lyric request; older responses are dropped so a slow
    /// fetch cannot overwrite a newer one (e.g. double-tapped refresh).
    lyrics_seq: u64,
    /// Track restored in a paused session. The first explicit play starts it
    /// at this position instead of from the beginning.
    pending_resume: Option<PendingResume>,
    /// Last values written to `playback.json`, used to throttle position
    /// saves while the player reports progress every 500 ms.
    saved_position: f64,
    saved_paused: bool,
    saved_once: bool,
}

struct PendingResume {
    bvid: String,
    page: usize,
    position: f64,
}

impl Session {
    /// Build the session. The message receiver is returned to the caller:
    /// it must drive `recv()` + `process()` from its own event loop (the TUI
    /// select loop, or the UniFFI pump task). Keeping the receiver outside
    /// `Session` lets foreign frontends wait for messages without holding
    /// any lock the command methods need.
    pub async fn new(
        api: Arc<ApiClient>,
        player: Arc<dyn Player>,
    ) -> (Self, mpsc::UnboundedReceiver<Msg>) {
        let (tx, rx) = mpsc::unbounded_channel();

        // Forward backend state snapshots into the message loop. The mpv
        // backend polls every 500 ms; ExoPlayer pushes on every change.
        let mut state_rx = player.watch();
        let relay_tx = tx.clone();
        tokio::spawn(async move {
            while state_rx.changed().await.is_ok() {
                let state = state_rx.borrow().clone();
                if relay_tx.send(Msg::PlayerState(state)).is_err() {
                    break;
                }
            }
        });

        let mut session = Self {
            api: api.clone(),
            player,
            app: App::new(),
            tx,
            lyrics_seq: 0,
            pending_resume: None,
            saved_position: 0.0,
            saved_paused: false,
            saved_once: false,
        };
        session.app.quality_pref = api.audio_quality();
        session.app.play_mode = api.play_mode();
        session.app.cache_mode = api.cache().mode();
        session.update_cache_stats();
        session.refresh_cached_tracks();
        session.restore_last_playback();
        spawn_user_info(&session.api, session.tx.clone());
        session.app.has_cookie = api.has_cookie().await;
        if session.app.has_cookie {
            // Show the cached library immediately; the network refresh below
            // replaces it when available. This keeps cached audio reachable
            // even when the app starts offline.
            if let Some(folders) = api.cached_favorite_folders().await {
                let hidden = api.hidden_playlists().await;
                session.app.load_cached_playlists(folders, hidden);
            }
            // Keep a restored queue/playlist visible; only fall back to the
            // folder list when there was nothing to restore.
            if session.app.results.is_empty() && session.app.now_playing.is_none() {
                session.app.open_library();
            }
            session.refresh_playlists();
        }
        (session, rx)
    }

    /// Load the last queue and track. Playback stays paused on startup; an
    /// explicit play or retry resumes from the saved position.
    fn restore_last_playback(&mut self) {
        let Some(saved) = crate::resume::load(self.api.data_dir()) else {
            return;
        };
        let request = saved.apply(&mut self.app);
        self.saved_position = self.app.position;
        self.saved_paused = self.app.paused;
        self.saved_once = true;

        // Push the paused track's cover through the same CoverReady event so
        // the UI shows it right away; `image_bytes` serves a cached copy from
        // disk when one exists.
        if let Some(item) = self.app.now_playing.clone()
            && !self.app.cover_key.is_empty()
        {
            spawn_cover(&self.api, self.tx.clone(), self.app.cover_key.clone(), item.pic);
        }

        if let Some(request) = request {
            // Kept until playback actually starts so the play button and the
            // retry action resume from the same position.
            self.pending_resume = Some(PendingResume {
                bvid: request.item.bvid.clone(),
                page: request.page_index,
                position: request.start_at,
            });
        }
    }

    // ----- commands -------------------------------------------------------

    pub fn search(&mut self, keyword: String) {
        self.app.search_seq += 1;
        let seq = self.app.search_seq;
        self.app.loading = true;
        self.app.loading_more = false;
        self.app.view = MainView::Queue;
        self.app.set_status("搜索中…", false);
        let _ = self.tx.send(Msg::StateChanged);
        spawn_search(&self.api, self.tx.clone(), seq, keyword);
    }

    /// Fetch the next page of the current search, if one exists.
    pub fn load_more_search(&mut self) {
        if self.app.loading || self.app.loading_more || !self.app.has_more_results() {
            return;
        }
        let seq = self.app.search_seq;
        let keyword = self.app.search_keyword.clone();
        let page = self.app.search_page + 1;
        self.app.loading_more = true;
        let _ = self.tx.send(Msg::StateChanged);
        spawn_search_more(&self.api, self.tx.clone(), seq, keyword, page);
    }

    pub fn play(&mut self, mut request: PlayRequest) {
        // A restored track resumes at its saved position when the user starts
        // it again (play button, retry, queue row). Any other track clears the
        // pending resume so it cannot affect later playback.
        if let Some(pending) = self.pending_resume.as_ref() {
            if pending.bvid == request.item.bvid && pending.page == request.page_index {
                if request.start_at <= 0.0 {
                    request.start_at = pending.position;
                }
            } else {
                self.pending_resume = None;
            }
        }
        spawn_play(
            &self.api,
            self.player.clone(),
            self.tx.clone(),
            request,
            self.app.quality_pref,
            self.app.vip,
        );
    }

    /// Play one entry from the offline cached list and make that list the
    /// current queue, so the Playing screen and next/previous have context.
    pub fn play_cached(&mut self, bvid: String) {
        let tracks = self.api.cache().cached_tracks();
        let Some(index) = tracks.iter().position(|track| track.bvid == bvid) else {
            self.app.set_status("缓存文件不存在，请重新在线播放", true);
            let _ = self.tx.send(Msg::StateChanged);
            return;
        };
        let target = tracks[index].clone();
        let items = tracks
            .iter()
            .map(|track| VideoItem {
                aid: track.aid,
                bvid: track.bvid.clone(),
                title: track.title.clone(),
                author: track.author.clone(),
                duration: track.duration.clone(),
                pic: track.pic.clone(),
            })
            .collect::<Vec<_>>();
        self.app.show_cached_tracks(items);
        let _ = self.tx.send(Msg::StateChanged);
        if let Some(mut request) = self.app.play_at(index) {
            request.page_index = target.page_index as usize;
            self.play(request);
        }
    }

    pub async fn pause(&mut self) {
        // Paused restores leave no media loaded, so the play button must
        // start the saved track rather than toggle an empty player.
        if let Some(request) = self.pending_resume_request() {
            self.app.pending_play = true;
            self.app.set_status("正在恢复上次播放…", false);
            self.play(request);
            return;
        }
        if !self.player.is_started().await {
            self.app.paused = !self.app.paused;
            return;
        }
        if let Err(error) = self.player.toggle_pause().await {
            self.app.paused = !self.app.paused;
            self.app.set_error(error);
        }
    }

    fn pending_resume_request(&self) -> Option<PlayRequest> {
        if self.app.has_media || self.app.pending_play {
            return None;
        }
        let pending = self.pending_resume.as_ref()?;
        let item = self.app.now_playing.clone()?;
        if item.bvid != pending.bvid || self.app.page != pending.page {
            return None;
        }
        Some(PlayRequest {
            item,
            page_index: pending.page,
            start_at: pending.position,
        })
    }

    pub async fn seek(&mut self, delta: f64) {
        if self.player.is_started().await
            && let Err(error) = self.player.seek(delta).await
        {
            self.app.set_error(error);
        }
    }

    pub async fn seek_to(&mut self, seconds: f64) {
        if self.player.is_started().await
            && let Err(error) = self.player.seek_to(seconds).await
        {
            self.app.set_error(error);
        }
    }

    pub async fn volume(&mut self, value: f64) {
        if self.player.is_started().await
            && let Err(error) = self.player.set_volume(value).await
        {
            self.app.set_error(error);
        }
    }

    /// Change the audio track preference and persist it for the next start.
    pub fn set_audio_quality(&mut self, quality: AudioQuality) {
        self.app.quality_pref = quality;
        self.api.save_audio_quality(quality);
        self.app
            .set_status(&quality_status(quality, self.app.vip), false);
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Change the playback order and persist it for the next start.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.app.play_mode = mode;
        if mode == PlayMode::Shuffle {
            self.app.reset_shuffle();
        }
        self.api.save_play_mode(mode);
        self.app
            .set_status(&format!("播放模式：{}", mode.label()), false);
        let _ = self.tx.send(Msg::StateChanged);
    }

    pub fn login(&self, seq: u64) {
        spawn_login(&self.api, self.tx.clone(), seq);
    }

    // ----- cache management ------------------------------------------------

    /// Recompute the cache usage for the UI. Cheap enough to call whenever a
    /// cover/lyrics/audio write may have changed it.
    fn update_cache_stats(&mut self) -> bool {
        let stats = self.api.cache().stats();
        if self.app.cache_stats == Some(stats) {
            return false;
        }
        self.app.cache_stats = Some(stats);
        true
    }

    /// Cached file of the track currently playing; cleanup must keep it so
    /// playback (and a pending seek) keeps working.
    fn playing_cached_audio(&self) -> Option<PathBuf> {
        if !self.app.cached_now {
            return None;
        }
        let item = self.app.now_playing.as_ref()?;
        self.api
            .cache()
            .find_audio(&item.bvid, self.app.current_cid)
            .map(|cached| cached.path)
    }

    /// Switch what the cache stores. Existing files are kept; turning audio
    /// caching back on makes them playable again without a re-download.
    pub fn set_cache_mode(&mut self, mode: CacheMode) {
        self.api.cache().set_mode(mode);
        self.app.cache_mode = mode;
        self.update_cache_stats();
        self.refresh_cached_tracks();
        self.app.set_status(
            &format!("缓存策略：{}（{}）", mode.label(), mode.description()),
            false,
        );
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Change the cache size cap at runtime. Lowering it evicts least
    /// recently used files immediately; 0 disables further caching.
    pub fn set_cache_limit_mb(&mut self, limit_mb: u64) {
        let keep = self.playing_cached_audio();
        let limit_bytes = self.api.cache().set_limit_mb(limit_mb, keep.as_deref());
        if limit_bytes == 0 {
            // Keep the exposed mode in sync with the effective behaviour.
            self.api.cache().set_mode(CacheMode::Off);
            self.app.cache_mode = CacheMode::Off;
        }
        self.update_cache_stats();
        self.refresh_cached_tracks();
        if limit_bytes == 0 {
            self.app
                .set_status("缓存已关闭，已有文件可以手动清理", false);
        } else {
            self.app.set_status(
                &format!("缓存上限已设为 {}", crate::cache::format_bytes(limit_bytes)),
                false,
            );
        }
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Recompute the cache usage and tell the frontend when it changed.
    pub fn refresh_cache_stats(&mut self) {
        let stats_changed = self.update_cache_stats();
        let markers_changed = self.refresh_cached_tracks();
        if stats_changed || markers_changed {
            let _ = self.tx.send(Msg::StateChanged);
        }
    }

    /// Re-scan completed audio files so list markers and the offline cached
    /// list follow cache changes.
    fn refresh_cached_tracks(&mut self) -> bool {
        let bvids = self.api.cache().cached_bvids();
        let tracks = self.api.cache().cached_tracks();
        if self.app.cached_bvids == bvids && self.app.cached_tracks == tracks {
            return false;
        }
        self.app.cached_bvids = bvids;
        self.app.cached_tracks = tracks;
        true
    }

    /// Delete cached audio, covers, lyrics and video metadata. The file of
    /// the track currently playing from cache is kept so playback (and a
    /// pending seek) keeps working.
    pub fn clear_cache(&mut self) {
        let keep = self.playing_cached_audio();
        let freed = self.api.cache().clear(keep.as_deref());
        self.update_cache_stats();
        self.refresh_cached_tracks();
        if freed > 0 {
            self.app.set_status(
                &format!("已清理缓存，释放 {}", crate::cache::format_bytes(freed)),
                false,
            );
        } else {
            self.app.set_status("缓存已经是空的", false);
        }
        let _ = self.tx.send(Msg::StateChanged);
    }

    // ----- playlist (favorite folder) commands ----------------------------

    /// Show the playlist library, loading folders when possible.
    pub async fn show_library(&mut self) {
        self.app.open_library();
        if !self.api.has_cookie().await {
            self.app.invalidate_playlist_requests();
            self.app.playlists.clear();
            self.app.set_status("登录后管理歌单", false);
            self.save_playback();
            return;
        }
        self.refresh_playlists();
        self.save_playback();
    }

    pub fn show_queue(&mut self) {
        self.app.open_queue();
        self.save_playback();
    }

    pub fn refresh_playlists(&mut self) {
        if self.app.playlists_loading {
            return;
        }
        let seq = self.app.begin_playlists_load();
        spawn_playlists(&self.api, self.tx.clone(), seq);
    }

    pub fn open_playlist(&mut self, folder: FavoriteFolder) {
        self.app
            .set_status(&format!("正在打开「{}」…", folder.title), false);
        let seq = self.app.begin_playlist_open();
        spawn_open_playlist(&self.api, self.tx.clone(), seq, folder);
    }

    pub fn create_playlist(&mut self, title: String) {
        let pending = self.app.take_pending_import();
        spawn_create_playlist(&self.api, self.tx.clone(), title, pending);
    }

    /// Drop the import that was waiting for a new playlist.
    pub fn cancel_pending_import(&mut self) {
        if self.app.pending_import.take().is_some() {
            self.app.set_status("已保留在本地列表", false);
            let _ = self.tx.send(Msg::StateChanged);
        }
    }

    pub fn rename_playlist(&self, folder_id: i64, title: String) {
        spawn_rename_playlist(&self.api, self.tx.clone(), folder_id, title);
    }

    pub fn delete_playlist(&self, folder_id: i64) {
        spawn_delete_playlist(&self.api, self.tx.clone(), folder_id);
    }

    /// Hide or unhide a favorite folder. The hidden list is persisted per
    /// account.
    pub fn set_playlist_hidden(&mut self, folder_id: i64, hidden: bool) {
        self.app.set_status(
            if hidden {
                "正在隐藏歌单…"
            } else {
                "正在恢复歌单…"
            },
            false,
        );
        let _ = self.tx.send(Msg::StateChanged);
        spawn_set_playlist_hidden(&self.api, self.tx.clone(), folder_id, hidden);
    }

    /// Temporarily reveal or collapse hidden folders in the library.
    pub fn toggle_show_hidden(&mut self) {
        self.app.toggle_show_hidden();
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Open the add-to-playlist picker, loading folders first if needed.
    pub fn begin_add_track(&mut self, bvid: String) {
        self.app.open_picker(bvid);
        if self.app.playlists.is_empty() && !self.app.playlists_loading {
            self.refresh_playlists();
        }
    }

    pub fn add_track_to_playlist(&mut self, folder_id: i64, bvid: String) {
        // Picking a folder from the import prompt is the user's choice; the
        // parked import has now been placed.
        self.app.cancel_pending_import();
        let item = self
            .app
            .results
            .iter()
            .find(|item| item.bvid == bvid)
            .cloned();
        spawn_add_track(&self.api, self.tx.clone(), folder_id, bvid, item);
    }

    pub fn remove_track_from_playlist(&self, folder_id: i64, bvid: String) {
        let item = self
            .app
            .results
            .iter()
            .find(|item| item.bvid == bvid)
            .cloned();
        spawn_remove_track(&self.api, self.tx.clone(), folder_id, bvid, item);
    }

    /// Import a BV id / link. With `target_open_playlist` the currently open
    /// playlist is the destination (TUI keyboard flow); otherwise the track
    /// only parks in the queue and the frontend asks which folder to use.
    pub fn import_bv(&mut self, raw: String, target_open_playlist: bool) {
        let playlist = target_open_playlist
            .then(|| self.app.current_playlist.clone())
            .flatten();
        spawn_import_bv(&self.api, self.tx.clone(), raw, playlist);
    }

    // ----- extra commands (keyboard shortcuts on the TUI) ------------------

    /// Advance to the previous/next track (TUI's `n`/`p`).
    pub fn step_track(&mut self, delta: isize) {
        if let Some(request) = self.app.step(delta) {
            self.play(request);
        }
    }

    /// Re-resolve the stream for the current track (TUI's `r`).
    pub fn retry_current(&mut self) {
        if self.app.pending_play {
            return;
        }
        let item = self.app.now_playing.clone().or_else(|| {
            self.app
                .current
                .and_then(|index| self.app.results.get(index))
                .cloned()
        });
        if let Some(item) = item {
            self.app.pending_play = true;
            self.app.set_status("重新获取播放地址…", false);
            self.play(PlayRequest {
                item,
                page_index: self.app.page,
                start_at: 0.0,
            });
        }
    }

    /// Jump straight to one 分P of the currently playing video (page picker).
    pub fn play_page(&mut self, page: usize) {
        if self.app.pending_play {
            return;
        }
        let Some(item) = self.app.now_playing.clone() else {
            self.app.set_status("还没有播放内容", true);
            return;
        };
        if page >= self.app.playing_pages.len() {
            self.app.set_status("没有这个分P", true);
            return;
        }
        if page == self.app.page {
            return;
        }
        self.app.pending_play = true;
        self.app.page = page;
        self.app.set_status("准备播放…", false);
        self.play(PlayRequest {
            item,
            page_index: page,
            start_at: 0.0,
        });
    }

    /// Re-fetch lyrics for the current track, bypassing (and dropping) the
    /// disk cache. Frontends expose this when the shown lyrics look wrong.
    pub fn refresh_lyrics(&mut self) {
        let Some(item) = self.app.now_playing.clone() else {
            self.app.set_status("还没有播放内容", true);
            return;
        };
        if self.app.lyrics_key.is_empty() {
            self.app.set_status("还没有播放内容", true);
            return;
        }
        self.app.lyrics.clear();
        self.app.lyrics_status = "重新加载歌词…".to_string();
        let seq = self.next_lyrics_seq();
        spawn_lyrics(
            &self.api,
            self.tx.clone(),
            self.app.lyrics_key.clone(),
            item.aid,
            item.bvid,
            self.app.current_cid,
            seq,
            true,
        );
    }

    fn next_lyrics_seq(&mut self) -> u64 {
        self.lyrics_seq = self.lyrics_seq.wrapping_add(1);
        self.lyrics_seq
    }

    /// Open the QR login modal and start polling (TUI's `g`).
    pub fn open_login(&mut self) {
        let seq = self.app.open_login();
        spawn_login(&self.api, self.tx.clone(), seq);
    }

    /// Close the login modal and invalidate the polling task.
    pub fn cancel_login(&mut self) {
        if self.app.login.is_some() {
            self.app.login_seq += 1;
            self.app.login = None;
            self.app.set_status("已取消登录", false);
        }
    }

    /// Install a cookie obtained outside the QR flow (in-app WebView or
    /// pasted by the user), then refresh the account state.
    pub async fn set_cookie_text(&mut self, cookie: String) {
        self.api.set_cookie(cookie).await;
        self.app.has_cookie = true;
        self.app.cancel_pending_import();
        match self.api.user_info().await {
            Ok(Some(user)) => {
                self.app.vip = user.vip;
                self.app.user_face = user.face;
                self.app.user = Some(user.name.clone());
                self.app
                    .set_status(&format!("已登录：{}", user.name), false);
            }
            Ok(None) => {
                self.app.vip = false;
                self.app
                    .set_status("Cookie 已保存，但未能确认登录状态", true);
            }
            Err(error) => {
                self.app
                    .set_status(&format!("Cookie 已保存，用户信息获取失败: {error:#}"), true);
            }
        }
        // Switching accounts must not keep the previous account's folders
        // visible; swap in the new account's cache (if any) until the refresh
        // lands.
        self.app.playlists.clear();
        if let Some(folders) = self.api.cached_favorite_folders().await {
            let hidden = self.api.hidden_playlists().await;
            self.app.load_cached_playlists(folders, hidden);
        }
        self.app.open_library();
        self.refresh_playlists();
    }

    /// Sign out: forget the cookie and the cached account.
    pub async fn logout(&mut self) {
        self.api.clear_cookie().await;
        self.app.cancel_pending_import();
        self.app.invalidate_playlist_requests();
        self.app.has_cookie = false;
        self.app.user = None;
        self.app.user_face = None;
        self.app.vip = false;
        self.app.playlists.clear();
        self.app.hidden_playlists.clear();
        self.app.show_hidden = false;
        self.app.playlist_selected = None;
        self.app.current_playlist = None;
        self.app.open_queue();
        self.app.set_status("已退出登录", false);
        self.save_playback();
    }

    /// Stop playback entirely.
    pub async fn stop(&mut self) {
        self.player.quit().await;
        self.app.has_media = false;
        self.app.paused = false;
        self.app.set_status("已停止播放", false);
        self.save_playback();
    }

    /// Persist the queue, current track and position for the next start.
    /// Errors are deliberately ignored: a read-only config directory must not
    /// break playback.
    pub fn save_playback(&mut self) {
        let saved = SavedPlayback::capture(&self.app);
        if crate::resume::save(self.api.data_dir(), &saved).is_ok() {
            self.saved_position = self.app.position;
            self.saved_paused = self.app.paused;
            self.saved_once = true;
        }
    }

    /// Write the current folder list to the per-account offline cache.
    fn persist_playlists(&self) {
        let api = self.api.clone();
        let folders = self.app.playlists.clone();
        tokio::spawn(async move {
            api.save_playlist_cache(&folders).await;
        });
    }

    /// Periodic progress updates every 500 ms are too frequent for disk;
    /// write once per interval (or immediately when pause state flips).
    fn progress_save_due(&self) -> bool {
        if !self.saved_once {
            return true;
        }
        (self.app.position - self.saved_position).abs() >= PROGRESS_SAVE_INTERVAL_SECS
            || self.saved_paused != self.app.paused
    }

    // ----- event pump -----------------------------------------------------

    /// Apply one message to the app state and report what the frontend should
    /// do. Sync and lock-free; also drives EOF auto-advance.
    pub fn process(&mut self, message: Msg) -> CoreEvent {
        // Messages that can add tracks to the visible list or write new audio
        // files; only those need a cache re-scan for the list markers.
        let refresh_cached = matches!(
            &message,
            Msg::SearchDone { .. }
                | Msg::SearchMoreDone { .. }
                | Msg::PlaylistOpened { .. }
                | Msg::BvImported(_)
                | Msg::CacheDone { .. }
                | Msg::Playing(_)
        );
        // Only queue/track changes are saved here; progress reports are
        // throttled separately and login/lyrics/cache chatter is skipped.
        let progress_event = matches!(&message, Msg::PlayerState(_));
        let save_after = matches!(
            &message,
            Msg::Playing(_)
                | Msg::PlayFailed(_)
                | Msg::SearchDone { .. }
                | Msg::SearchMoreDone { .. }
                | Msg::PlaylistOpened { .. }
                | Msg::BvImported(_)
                | Msg::TrackRemoved { .. }
                | Msg::PlaylistDeleted { .. }
                | Msg::PlaylistRenamed { .. }
        );
        // Folder mutations and count changes should survive an offline restart.
        let persist_after = matches!(
            &message,
            Msg::PlaylistCreated { .. }
                | Msg::PlaylistRenamed { .. }
                | Msg::PlaylistDeleted { .. }
                | Msg::TrackAdded { .. }
                | Msg::TrackRemoved { .. }
                | Msg::BvImported(_)
        );
        let playback_started = matches!(message, Msg::Playing(_));
        match message {
            Msg::StateChanged => {}
            Msg::SearchDone {
                seq,
                keyword,
                result,
            } => self.app.on_search_done(seq, keyword, result),
            Msg::SearchMoreDone { seq, result } => self.app.on_search_more(seq, result),
            Msg::Playing(info) => {
                let key = format!("{}#{}#{}", info.item.bvid, info.cid, info.page_index);
                let aid = info.item.aid;
                let bvid = info.item.bvid.clone();
                let cid = info.cid;
                let pic = info.pic.clone();
                self.app.on_playing(info);
                spawn_cover(&self.api, self.tx.clone(), key, pic);
                let seq = self.next_lyrics_seq();
                spawn_lyrics(
                    &self.api,
                    self.tx.clone(),
                    self.app.lyrics_key.clone(),
                    aid,
                    bvid,
                    cid,
                    seq,
                    false,
                );
            }
            Msg::PlayFailed(error) => {
                self.app.on_play_failed(error);
                // Offline queues often mix cached and uncached tracks: skip
                // the unplayable entry and keep looking for the next one.
                if let Some(request) = self.app.skip_failed() {
                    self.play(request);
                } else {
                    let _ = self.tx.send(Msg::StateChanged);
                }
            }
            Msg::PlayerState(state) => self.app.on_player_state(state),
            Msg::LoginQr { seq, matrix } => self.app.on_login_qr(seq, matrix),
            Msg::LoginPhase { seq, status } => self.app.on_login_phase(seq, status),
            Msg::LoginSuccess { seq, user } => {
                if self.app.on_login_success(seq, user) {
                    self.app.has_cookie = true;
                    self.app.playlists.clear();
                    if let Some(item) = self.app.now_playing.clone() {
                        // Subtitles often require login; re-fetch for the
                        // current track so a newly available track wins over
                        // the anonymous one.
                        let seq = self.next_lyrics_seq();
                        spawn_lyrics(
                            &self.api,
                            self.tx.clone(),
                            self.app.lyrics_key.clone(),
                            item.aid,
                            item.bvid,
                            self.app.current_cid,
                            seq,
                            true,
                        );
                    }
                    self.refresh_playlists();
                }
            }
            Msg::LoginFailed { seq, message } => self.app.on_login_failed(seq, message),
            Msg::PlaylistsLoaded { seq, result } => self.app.on_playlists_loaded(seq, result),
            Msg::PlaylistOpened {
                seq,
                folder,
                result,
                offline,
            } => self.app.on_playlist_opened(seq, folder, result, offline),
            Msg::PlaylistCreated { result, pending } => {
                let created = result
                    .as_ref()
                    .ok()
                    .map(|folder| (folder.id, folder.title.clone()));
                self.app.on_playlist_created(result);
                if let (Some((folder_id, folder_title)), Some(pending)) = (created, pending) {
                    self.app.set_status(
                        &format!(
                            "已新建歌单「{folder_title}」，正在加入「{}」…",
                            pending.title
                        ),
                        false,
                    );
                    self.add_track_to_playlist(folder_id, pending.bvid);
                }
            }
            Msg::PlaylistRenamed {
                folder_id,
                title,
                result,
            } => self.app.on_playlist_renamed(folder_id, title, result),
            Msg::PlaylistDeleted { folder_id, result } => {
                self.app.on_playlist_deleted(folder_id, result)
            }
            Msg::PlaylistHidden {
                folder_id,
                hidden,
                result,
            } => self.app.on_playlist_hidden(folder_id, hidden, result),
            Msg::TrackAdded {
                folder_id,
                changed,
                result,
            } => self.app.on_track_added(folder_id, changed, result),
            Msg::TrackRemoved {
                folder_id,
                bvid,
                changed,
                result,
            } => self.app.on_track_removed(folder_id, bvid, changed, result),
            Msg::CoverReady { key, result } => {
                let cached = result.is_ok();
                if key == self.app.cover_key
                    && let Ok(bytes) = result
                {
                    if cached {
                        self.update_cache_stats();
                    }
                    return CoreEvent::CoverReady { key, bytes };
                }
            }
            Msg::CacheProgress {
                key,
                downloaded,
                total,
            } => self.app.on_cache_progress(key, downloaded, total),
            Msg::CacheDone { key, ok } => {
                self.app.on_cache_done(key, ok);
                self.update_cache_stats();
            }
            Msg::LyricsReady { seq, key, result } => {
                if seq == self.lyrics_seq {
                    let cached = result.is_ok();
                    self.app.on_lyrics(key, result);
                    if cached {
                        self.update_cache_stats();
                    }
                }
            }
            Msg::BvImported(result) => {
                self.app.on_bv_imported(result);
                // A parked import now asks which folder should receive it;
                // the picker renders the folders (and a "new playlist" row)
                // as soon as they are available.
                if let Some(pending) = self.app.pending_import.clone() {
                    self.begin_add_track(pending.bvid);
                }
            }
            Msg::UserInfo(user) => {
                // Startup-only probe: ignore it if a login/cookie flow already
                // filled the account, or a slow response would clobber VIP.
                if self.app.user.is_none() {
                    self.app.vip = user.as_ref().is_some_and(|user| user.vip);
                    if let Some(user) = user {
                        self.app.user_face = user.face;
                        self.app.user = Some(user.name);
                    }
                }
            }
        }

        if persist_after {
            self.persist_playlists();
        }
        if playback_started {
            self.pending_resume = None;
        }
        if refresh_cached {
            self.refresh_cached_tracks();
        }

        // Auto-advance once per track end. take_eof() clears the flag so this
        // cannot loop on the same track. Search results are pick-one lists:
        // they keep playing pages within one video but stop afterwards.
        if self.app.take_eof() && !self.app.pending_play {
            if let Some(request) = self.app.auto_advance() {
                self.play(request);
            } else {
                self.app.has_media = false;
                self.app.set_status("播放结束", false);
            }
        }

        let save_now = if progress_event {
            self.progress_save_due()
        } else {
            save_after
        };
        if save_now {
            self.save_playback();
        }

        CoreEvent::StateChanged
    }
}

// ----- background tasks ---------------------------------------------------

/// One-line explanation of what a preference resolves to for this account.
fn quality_status(quality: AudioQuality, vip: bool) -> String {
    let hint = match (quality, vip) {
        (AudioQuality::Auto, true) => "大会员：优先 Hi-Res/Dolby",
        (AudioQuality::Auto, false) => "最高 192K AAC",
        (AudioQuality::Aac, _) => "最高 192K AAC",
        (AudioQuality::Dolby, true) => "优先 Dolby，不可用时回退 AAC",
        (AudioQuality::Dolby, false) => "需要大会员，无权益时回退 AAC",
        (AudioQuality::HiRes, true) => "优先 Hi-Res FLAC，不可用时回退 AAC",
        (AudioQuality::HiRes, false) => "需要大会员，无权益时回退 AAC",
    };
    format!("音质偏好：{}（{hint}；切歌后生效）", quality.label())
}

fn spawn_user_info(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>) {
    let api = api.clone();
    tokio::spawn(async move {
        // The first request right after process start can lose to a slow
        // network; retry a couple of times before giving up on the name.
        let mut user = None;
        for attempt in 0..3 {
            match api.user_info().await {
                Ok(info) => {
                    user = info;
                    break;
                }
                Err(_) if attempt < 2 => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(Msg::UserInfo(user));
    });
}

fn spawn_search(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, seq: u64, keyword: String) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .search_page(&keyword, 1)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::SearchDone {
            seq,
            keyword,
            result,
        });
    });
}

fn spawn_search_more(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    seq: u64,
    keyword: String,
    page: u32,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .search_page(&keyword, page)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::SearchMoreDone { seq, result });
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
                        status: "二维码已过期，重新打开登录".to_string(),
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
            status: "二维码已过期，重新打开登录".to_string(),
        });
    });
}

fn spawn_playlists(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, seq: u64) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let folders = api.favorite_folders().await?;
            let hidden = api.hidden_playlists().await;
            Ok::<_, anyhow::Error>((folders, hidden))
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistsLoaded { seq, result });
    });
}

fn spawn_set_playlist_hidden(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    hidden: bool,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .set_playlist_hidden(folder_id, hidden)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistHidden {
            folder_id,
            hidden,
            result,
        });
    });
}

fn spawn_open_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    seq: u64,
    folder: FavoriteFolder,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let (result, offline) = match api.favorite_items(folder.id).await {
            Ok(items) => (Ok(items), false),
            Err(error) => match api.cached_favorite_items(folder.id).await {
                Some(items) => (Ok(items), true),
                None => (Err(format!("{error:#}")), false),
            },
        };
        let _ = tx.send(Msg::PlaylistOpened {
            seq,
            folder,
            result,
            offline,
        });
    });
}

fn spawn_create_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    title: String,
    pending: Option<PendingImport>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .create_playlist(&title)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistCreated { result, pending });
    });
}

fn spawn_rename_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    title: String,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .rename_playlist(folder_id, &title)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistRenamed {
            folder_id,
            title,
            result,
        });
    });
}

fn spawn_delete_playlist(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, folder_id: i64) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .delete_playlist(folder_id)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistDeleted { folder_id, result });
    });
}

/// Resolve the aid without a `view` request whenever the item already has it.
async fn resolve_aid(api: &ApiClient, item: Option<&VideoItem>, bvid: &str) -> Result<i64> {
    if let Some(item) = item
        && item.aid > 0
    {
        return Ok(item.aid);
    }
    let aid = api.view(bvid).await?.aid;
    if aid == 0 {
        bail!("视频信息缺少 aid");
    }
    Ok(aid)
}

fn spawn_add_track(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    bvid: String,
    item: Option<VideoItem>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let aid = resolve_aid(&api, item.as_ref(), &bvid).await?;
            api.set_favorite(folder_id, aid, true).await
        }
        .await;
        let (changed, result) = match result {
            Ok(changed) => (changed, Ok(())),
            Err(error) => (false, Err(format!("{error:#}"))),
        };
        let _ = tx.send(Msg::TrackAdded {
            folder_id,
            changed,
            result,
        });
    });
}

fn spawn_remove_track(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    bvid: String,
    item: Option<VideoItem>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let aid = resolve_aid(&api, item.as_ref(), &bvid).await?;
            api.set_favorite(folder_id, aid, false).await
        }
        .await;
        let (changed, result) = match result {
            Ok(changed) => (changed, Ok(())),
            Err(error) => (false, Err(format!("{error:#}"))),
        };
        let _ = tx.send(Msg::TrackRemoved {
            folder_id,
            bvid,
            changed,
            result,
        });
    });
}

fn spawn_import_bv(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    raw: String,
    playlist: Option<FavoriteFolder>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = import_bv(&api, &raw, playlist)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::BvImported(result));
    });
}

async fn import_bv(
    api: &ApiClient,
    raw: &str,
    playlist: Option<FavoriteFolder>,
) -> Result<BvImport> {
    let bvid = match parse_bvid(raw) {
        Some(bvid) => bvid,
        None => {
            // Full links carry a BV id; short links (`b23.tv/...`, and the
            // share text wrapping them) only expose it after the redirect.
            let link = share_link(raw).context("无法识别 BV 号或链接")?;
            let resolved = api
                .resolve_share_link(link)
                .await
                .context("解析短链接失败")?;
            parse_bvid(&resolved).with_context(|| format!("链接中未找到 BV 号: {resolved}"))?
        }
    };
    let detail = api.view(&bvid).await?;
    if detail.aid == 0 {
        bail!("视频信息缺少 aid，无法加入歌单");
    }

    let total_duration = detail.pages.iter().map(|page| page.duration).sum::<i64>();
    let duration = if total_duration > 0 {
        total_duration
    } else {
        detail.duration
    };
    let item = VideoItem {
        aid: detail.aid,
        bvid: detail.bvid.clone(),
        title: detail.title.clone(),
        author: detail.author.clone(),
        duration: format_secs(duration),
        pic: detail.pic.clone(),
    };

    let logged_in = api.has_cookie().await;
    // A playlist opened in the UI is the natural destination; without one the
    // track is parked so the frontend can prompt for a new playlist.
    let (message, error, playlist) = if let Some(folder) = playlist {
        match api.set_favorite(folder.id, detail.aid, true).await {
            Ok(changed) => {
                let message = if changed {
                    format!("已加入歌单「{}」", folder.title)
                } else {
                    format!("已在歌单「{}」中", folder.title)
                };
                (message, false, changed.then_some(folder))
            }
            Err(error) => (format!("加入歌单失败: {error:#}"), true, None),
        }
    } else if logged_in {
        ("等待选择歌单".to_string(), false, None)
    } else {
        ("未登录，仅加入本地列表".to_string(), false, None)
    };

    Ok(BvImport {
        item,
        message,
        error,
        playlist,
        logged_in,
    })
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
        let result = api
            .image_bytes(&pic)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::CoverReady { key, result });
    });
}

fn spawn_lyrics(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    key: String,
    aid: i64,
    bvid: String,
    cid: i64,
    seq: u64,
    refresh: bool,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = if refresh {
            api.refresh_lyrics(aid, &bvid, cid).await
        } else {
            api.lyrics(aid, &bvid, cid).await
        }
        .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::LyricsReady { seq, key, result });
    });
}

/// Title a frontend should show for the loaded track. Multi-P videos play
/// one 分P per track and the part title is the actual song name, so it wins
/// over the collection-style video title.
fn track_display_title(video_title: &str, part: &str, page_count: usize) -> String {
    if page_count > 1 && !part.is_empty() {
        clean_title(part)
    } else {
        clean_title(video_title)
    }
}

fn spawn_play(
    api: &Arc<ApiClient>,
    player: Arc<dyn Player>,
    tx: mpsc::UnboundedSender<Msg>,
    request: PlayRequest,
    quality: AudioQuality,
    vip: bool,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let PlayRequest {
            item,
            page_index,
            start_at,
        } = request;
        let item_pic = item.pic.clone();
        let result: Result<(VideoDetail, AudioSource, usize, usize, String, i64, Vec<PageSummary>)> =
            async {
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
            let summaries = pages
                .iter()
                .enumerate()
                .map(|(position, page)| PageSummary {
                    cid: page.cid,
                    title: if page.part.is_empty() {
                        format!("P{}", position + 1)
                    } else {
                        page.part.clone()
                    },
                    duration: format_secs(page.duration),
                })
                .collect();

            let source = api
                .audio_source(&detail.bvid, page.cid, quality, vip)
                .await?;
            let target = source.target();
            let config = PlayerConfig {
                user_agent: crate::api::UA.to_string(),
                referer: crate::api::REFERER.to_string(),
                title: track_display_title(&detail.title, &page.part, pages.len()),
                artist: detail.author.clone(),
                cover_url: detail
                    .pic
                    .clone()
                    .or_else(|| item_pic.clone())
                    .as_deref()
                    .map(cover_thumbnail_url),
                start_position: start_at.max(0.0),
            };
            player.load(&target, &config).await?;
            Ok((
                detail,
                source,
                index,
                pages.len(),
                page.part.clone(),
                page.duration,
                summaries,
            ))
        }
        .await;

        match result {
            Ok((detail, source, page_index, page_count, part, page_duration, summaries)) => {
                let pic = detail.pic.clone().or(item_pic);
                let cid = detail
                    .pages
                    .get(page_index)
                    .map(|page| page.cid)
                    .unwrap_or(detail.cid);
                let bvid = detail.bvid.clone();
                let aid = detail.aid;
                let cached = source.is_cached();
                let quality = format!(
                    "{} · {} kbps",
                    quality_label(source.quality_id()),
                    source.bandwidth() / 1000
                );
                let playing = VideoItem {
                    aid,
                    bvid: detail.bvid,
                    title: track_display_title(&detail.title, &part, page_count),
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
                    pages: summaries,
                    start_position: start_at.max(0.0),
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
    if !api.audio_cache_enabled() {
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

/// Extract the first BV id from arbitrary text: a bare id or a video URL.
pub fn parse_bvid(input: &str) -> Option<String> {
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

/// Extract the first URL from pasted text. The app's share sheet wraps the
/// short link in a title (`【标题 - 哔哩哔哩】 https://b23.tv/xxxx`) and mobile
/// text can glue punctuation to the end, so stop at whitespace or CJK marks.
fn share_link(input: &str) -> Option<&str> {
    let start = ["https://", "http://"]
        .iter()
        .filter_map(|prefix| input.find(prefix))
        .min()?;
    let url = &input[start..];
    let end = url
        .find(|character: char| {
            character.is_whitespace() || matches!(character, '】' | '，' | '。' | '、')
        })
        .unwrap_or(url.len());
    Some(&url[..end])
}

#[cfg(test)]
mod tests {
    use super::{parse_bvid, share_link, track_display_title};

    #[test]
    fn parses_bvid_from_share_text() {
        let text = "【标题 - 哔哩哔哩】 https://www.bilibili.com/video/BV1xx411c7mD?p=2";
        assert_eq!(parse_bvid(text).as_deref(), Some("BV1xx411c7mD"));
    }

    #[test]
    fn multi_page_display_title_prefers_the_part_name() {
        assert_eq!(
            track_display_title("某合集", "第二话 歌名", 3),
            "第二话 歌名"
        );
        // No part name (or a single-P video) falls back to the video title,
        // which is cleaned of markup like search results are.
        assert_eq!(track_display_title("单曲", "", 1), "单曲");
        assert_eq!(track_display_title("某合集", "", 3), "某合集");
        assert_eq!(track_display_title("A&amp;B", "", 1), "A&B");
    }

    #[test]
    fn short_link_exposes_url_but_no_bvid() {
        let text = "【标题 - 哔哩哔哩】 https://b23.tv/AbCdEf】";
        assert_eq!(parse_bvid(text), None);
        assert_eq!(share_link(text), Some("https://b23.tv/AbCdEf"));
    }

    #[test]
    fn share_link_stops_at_punctuation() {
        assert_eq!(
            share_link("http://b23.tv/AbCdEf，看看这个"),
            Some("http://b23.tv/AbCdEf")
        );
        assert_eq!(share_link("没有链接"), None);
    }
}
