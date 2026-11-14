//! Message dispatch: routes every `Msg` into the `App` state machine.

use crate::session::Session;
use crate::session::msg::{CoreEvent, Msg};
use crate::session::tasks::cover::spawn_cover;
use crate::session::tasks::lyrics::spawn_lyrics;

impl Session {
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
