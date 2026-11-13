//! The session loop: owns the app state, dispatches background-task results
//! into it and drives auto-advance.
//!
//! Frontends feed it user commands (search/play/pause/...) and pump events
//! out. Every state mutation goes through `App::on_*` exactly like the
//! original TUI loop did, so both frontends observe identical behaviour.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc;

use crate::api::{
    ApiClient, AudioStream, AudioSource, QrPhase, VideoDetail, VideoItem, VideoPage, format_secs,
    quality_label,
};
use crate::app::{App, BvImport, PlayRequest, PlayingInfo, qr_matrix};
use crate::player::{Player, PlayerConfig};

/// Internal message vocabulary. `Key`/`Quit`/`Wake` stayed frontend-side;
/// everything else is session business.
pub enum Msg {
    SearchDone {
        seq: u64,
        result: Result<Vec<VideoItem>, String>,
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
    FoldersLoaded(Result<Vec<crate::api::FavoriteFolder>, String>),
    FavoritesImported {
        folder_title: String,
        result: Result<Vec<VideoItem>, String>,
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
        key: String,
        result: Result<Vec<crate::api::LyricLine>, String>,
    },
    UserInfo(Option<crate::api::UserInfo>),
    BvImported(Result<BvImport, String>),
    SyncPulled {
        auto: bool,
        result: Result<Option<(String, Vec<VideoItem>)>, String>,
    },
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
}

impl Session {
    /// Build the session. The message receiver is returned to the caller:
    /// it must drive `recv()` + `process()` from its own event loop (the TUI
    /// select loop, or the UniFFI pump task). Keeping the receiver outside
    /// `Session` lets foreign frontends wait for messages without holding
    /// any lock the command methods need.
    pub async fn new(api: Arc<ApiClient>, player: Arc<dyn Player>) -> (Self, mpsc::UnboundedReceiver<Msg>) {
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

        let session = Self {
            api: api.clone(),
            player,
            app: App::new(),
            tx,
        };
        spawn_user_info(&session.api, session.tx.clone());
        if api.has_cookie().await {
            spawn_sync_pull(&session.api, session.tx.clone(), true);
        }
        (session, rx)
    }

    // ----- commands -------------------------------------------------------

    pub fn search(&self, keyword: String) {
        let seq = self.app.search_seq;
        spawn_search(&self.api, self.tx.clone(), seq, keyword);
    }

    pub fn play(&self, request: PlayRequest) {
        spawn_play(&self.api, self.player.clone(), self.tx.clone(), request);
    }

    pub async fn pause(&mut self) {
        if !self.player.is_started().await {
            self.app.paused = !self.app.paused;
            return;
        }
        if let Err(error) = self.player.toggle_pause().await {
            self.app.paused = !self.app.paused;
            self.app.set_error(error);
        }
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

    pub fn login(&self, seq: u64) {
        spawn_login(&self.api, self.tx.clone(), seq);
    }

    pub async fn favorites_open(&mut self) {
        if self.api.has_cookie().await {
            self.app.open_favorites();
            spawn_folders(&self.api, self.tx.clone());
        } else {
            self.app.set_status("请先登录", true);
        }
    }

    pub fn import_folder(&self, folder: crate::api::FavoriteFolder) {
        spawn_import_folder(&self.api, self.tx.clone(), folder);
    }

    pub fn import_bv(&self, raw: String) {
        spawn_import_bv(&self.api, self.tx.clone(), raw);
    }

    pub async fn sync_pull(&mut self, auto: bool) {
        if auto {
            spawn_sync_pull(&self.api, self.tx.clone(), true);
            return;
        }
        if !self.api.has_cookie().await {
            self.app.set_status("请先登录", true);
        } else {
            self.app.set_status("正在从云同步加载…", false);
            spawn_sync_pull(&self.api, self.tx.clone(), false);
        }
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
            });
        }
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

    /// Close the favorites modal without importing.
    pub fn close_favorites(&mut self) {
        self.app.favorites = None;
    }

    /// Install a cookie obtained outside the QR flow (in-app WebView or
    /// pasted by the user), then refresh the account state.
    pub async fn set_cookie_text(&mut self, cookie: String) {
        self.api.set_cookie(cookie).await;
        match self.api.user_info().await {
            Ok(Some(user)) => {
                self.app.user = Some(user.name.clone());
                self.app
                    .set_status(&format!("已登录：{}", user.name), false);
            }
            Ok(None) => {
                self.app
                    .set_status("Cookie 已保存，但未能确认登录状态", true);
            }
            Err(error) => {
                self.app
                    .set_status(&format!("Cookie 已保存，用户信息获取失败: {error:#}"), true);
            }
        }
        spawn_sync_pull(&self.api, self.tx.clone(), true);
    }

    /// Stop playback entirely.
    pub async fn stop(&mut self) {
        self.player.quit().await;
        self.app.has_media = false;
        self.app.paused = false;
        self.app.set_status("已停止播放", false);
    }

    // ----- event pump -----------------------------------------------------

    /// Apply one message to the app state and report what the frontend should
    /// do. Sync and lock-free; also drives EOF auto-advance.
    pub fn process(&mut self, message: Msg) -> CoreEvent {
        match message {
            Msg::SearchDone { seq, result } => self.app.on_search_done(seq, result),
            Msg::Playing(info) => {
                let key = format!("{}#{}#{}", info.item.bvid, info.cid, info.page_index);
                let bvid = info.item.bvid.clone();
                let cid = info.cid;
                let pic = info.pic.clone();
                self.app.on_playing(info);
                spawn_cover(&self.api, self.tx.clone(), key, pic);
                spawn_lyrics(&self.api, self.tx.clone(), self.app.lyrics_key.clone(), bvid, cid);
            }
            Msg::PlayFailed(error) => self.app.on_play_failed(error),
            Msg::PlayerState(state) => self.app.on_player_state(state),
            Msg::LoginQr { seq, matrix } => self.app.on_login_qr(seq, matrix),
            Msg::LoginPhase { seq, status } => self.app.on_login_phase(seq, status),
            Msg::LoginSuccess { seq, user } => {
                if self.app.on_login_success(seq, user)
                    && let Some(item) = self.app.now_playing.clone()
                {
                    // Subtitles often require login; retry for the current track.
                    spawn_lyrics(
                        &self.api,
                        self.tx.clone(),
                        self.app.lyrics_key.clone(),
                        item.bvid,
                        self.app.current_cid,
                    );
                }
                spawn_sync_pull(&self.api, self.tx.clone(), true);
            }
            Msg::LoginFailed { seq, message } => self.app.on_login_failed(seq, message),
            Msg::FoldersLoaded(result) => self.app.on_folders_loaded(result),
            Msg::FavoritesImported {
                folder_title,
                result,
            } => self.app.on_favorites_imported(folder_title, result),
            Msg::CoverReady { key, result } => {
                if key == self.app.cover_key
                    && let Ok(bytes) = result
                {
                    return CoreEvent::CoverReady { key, bytes };
                }
            }
            Msg::CacheProgress {
                key,
                downloaded,
                total,
            } => self.app.on_cache_progress(key, downloaded, total),
            Msg::CacheDone { key, ok } => self.app.on_cache_done(key, ok),
            Msg::LyricsReady { key, result } => self.app.on_lyrics(key, result),
            Msg::BvImported(result) => self.app.on_bv_imported(result),
            Msg::SyncPulled { auto, result } => self.app.on_sync_pulled(auto, result),
            Msg::UserInfo(user) => {
                if self.app.user.is_none()
                    && let Some(user) = user
                {
                    self.app.user = Some(user.name);
                }
            }
        }

        // Auto-advance once per track end. take_eof() clears the flag so this
        // cannot loop on the same track. Search results are pick-one lists:
        // they keep playing pages within one video but stop afterwards.
        if self.app.take_eof() && !self.app.pending_play {
            if let Some(request) = self.app.auto_advance() {
                self.play(request);
            } else {
                self.app.has_media = false;
                let message = if self.app.queue_auto_advance {
                    "播放列表结束"
                } else {
                    "播放结束"
                };
                self.app.set_status(message, false);
            }
        }

        CoreEvent::StateChanged
    }
}

// ----- background tasks ---------------------------------------------------

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
    folder: crate::api::FavoriteFolder,
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

fn spawn_cover(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>, key: String, pic: Option<String>) {
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
    player: Arc<dyn Player>,
    tx: mpsc::UnboundedSender<Msg>,
    request: PlayRequest,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let PlayRequest { item, page_index } = request;
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
            let config = PlayerConfig {
                user_agent: crate::api::UA.to_string(),
                referer: crate::api::REFERER.to_string(),
            };
            player.load(&target, &config).await?;
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
