//! Snapshot construction: mirrors `App` state into the exported record.

use super::types::{CacheProgress, CacheUsage, Snapshot};
use crate::app::App;

pub(super) fn build_snapshot(app: &App) -> Snapshot {
    Snapshot {
        queue_title: app.results_title.clone(),
        results: app.results.clone(),
        selected: app.selected.map(|value| value as u32),
        current: app.current.map(|value| value as u32),
        status: app.status.clone(),
        error: app.error,
        loading: app.loading,
        has_more_results: app.has_more_results(),
        loading_more: app.loading_more,
        search_total: app.search_total,
        now_playing: app.now_playing.clone(),
        quality: app.quality.clone(),
        page: app.page as u32,
        page_count: app.page_count as u32,
        part: app.part.clone(),
        pages: app.playing_pages.clone(),
        position: app.position,
        duration: app.duration,
        paused: app.paused,
        volume: app.volume,
        has_media: app.has_media,
        pending_play: app.pending_play,
        has_cookie: app.has_cookie,
        user: app.user.clone(),
        user_face: app.user_face.clone(),
        vip: app.vip,
        quality_pref: app.quality_pref,
        play_mode: app.play_mode,
        login: app.login.clone(),
        in_library: app.view == crate::app::MainView::Library,
        playlists: app.playlists.clone(),
        playlists_loading: app.playlists_loading,
        hidden_playlist_ids: {
            let mut ids = app.hidden_playlists.iter().copied().collect::<Vec<_>>();
            ids.sort_unstable();
            ids
        },
        show_hidden: app.show_hidden,
        pending_import_title: app
            .pending_import
            .as_ref()
            .map(|pending| pending.title.clone()),
        pending_import_bvid: app
            .pending_import
            .as_ref()
            .map(|pending| pending.bvid.clone()),
        current_playlist: app.current_playlist.clone(),
        show_lyrics: app.show_lyrics,
        lyrics: app.lyrics.clone(),
        lyrics_status: app.lyrics_status.clone(),
        cover_key: app.cover_key.clone(),
        cache_progress: app
            .cache_progress
            .map(|(downloaded, total)| CacheProgress { downloaded, total }),
        cached_now: app.cached_now,
        cache_mode: app.cache_mode,
        cached_bvids: {
            let mut bvids = app.cached_bvids.iter().cloned().collect::<Vec<_>>();
            bvids.sort();
            bvids
        },
        cached_tracks: app.cached_tracks.clone(),
        cache_usage: app.cache_stats.map(|stats| CacheUsage {
            used_bytes: stats.total_bytes,
            limit_bytes: stats.limit_bytes,
            audio_bytes: stats.audio_bytes,
            cover_bytes: stats.cover_bytes,
            lyrics_bytes: stats.lyrics_bytes,
            view_bytes: stats.view_bytes,
            file_count: stats.file_count,
        }),
    }
}
