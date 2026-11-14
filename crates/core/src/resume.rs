//! Last-playback state persisted across runs.
//!
//! The queue, the playlist it was opened from, the current track and the
//! playback position are written to `playback.json` next to the cookie. On the
//! next start the session restores them instead of showing an empty player, so
//! the user does not have to find the same playlist and song again.
//!
//! The file is plain JSON so a bad write can be inspected by hand; writes go
//! through a temporary file and a rename so a crash never leaves a truncated
//! queue behind.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::{FavoriteFolder, VideoItem};
use crate::app::{App, MainView, PageSummary, PlayRequest};

pub const PLAYBACK_FILE_NAME: &str = "playback.json";

const VERSION: u32 = 1;
/// Playback shorter than this is not worth resuming; start the track over.
const MIN_RESUME_SECS: f64 = 3.0;
/// Do not resume into the closing seconds, where the track would end at once.
const END_GUARD_SECS: f64 = 3.0;

/// Everything needed to rebuild the last queue and continue playback.
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedPlayback {
    version: u32,
    queue_title: String,
    playlist: Option<FavoriteFolder>,
    items: Vec<VideoItem>,
    current: Option<usize>,
    page: usize,
    page_count: usize,
    part: String,
    /// 分P list of the loaded video, so the page picker works right after a
    /// restore (even offline). Old files without the field restore empty.
    #[serde(default)]
    pages: Vec<PageSummary>,
    quality: String,
    position: f64,
    duration: f64,
    in_library: bool,
    search_keyword: String,
    search_page: u32,
    search_pages: u32,
    search_total: u64,
    now_playing: Option<VideoItem>,
}

impl SavedPlayback {
    /// Snapshot the queue-related parts of `app`.
    pub fn capture(app: &App) -> Self {
        Self {
            version: VERSION,
            queue_title: app.results_title.clone(),
            playlist: app.current_playlist.clone(),
            items: app.results.clone(),
            current: app.current,
            page: app.page,
            page_count: app.page_count,
            part: app.part.clone(),
            pages: app.playing_pages.clone(),
            quality: app.quality.clone(),
            position: app.position.max(0.0),
            duration: app.duration.max(0.0),
            in_library: app.view == MainView::Library,
            search_keyword: app.search_keyword.clone(),
            search_page: app.search_page,
            search_pages: app.search_pages,
            search_total: app.search_total,
            now_playing: app.now_playing.clone(),
        }
    }

    /// Apply the saved queue to `app`. Returns the request that resumes the
    /// saved track when the user presses play; startup itself stays paused so
    /// a fresh launch never starts audio by surprise.
    pub fn apply(&self, app: &mut App) -> Option<PlayRequest> {
        if self.version != VERSION {
            return None;
        }

        app.results = self.items.clone();
        app.results_title = if self.queue_title.trim().is_empty() {
            match &self.playlist {
                Some(folder) => format!("歌单 · {}", folder.title),
                None => "播放队列".to_string(),
            }
        } else {
            self.queue_title.clone()
        };
        app.current_playlist = self.playlist.clone();
        app.current = self.current.filter(|index| *index < app.results.len());
        app.selected = app
            .current
            .or_else(|| (!app.results.is_empty()).then_some(0));
        app.page = if app.current.is_some() { self.page } else { 0 };
        app.page_count = self.page_count;
        app.part = self.part.clone();
        // The pages belong to the restored track; without a track they are
        // meaningless and the page picker stays hidden.
        app.playing_pages = if app.current.is_some() {
            self.pages.clone()
        } else {
            Vec::new()
        };
        app.quality = self.quality.clone();
        app.now_playing = self.now_playing.clone().or_else(|| {
            app.current
                .and_then(|index| app.results.get(index))
                .cloned()
        });
        app.position = self.position.max(0.0);
        app.duration = self.duration.max(0.0);
        app.has_media = false;
        app.playback_started = false;
        app.eof = false;
        // Startup restores the session paused; the first play starts the
        // track at the saved position.
        app.paused = app.now_playing.is_some();
        app.view = if self.in_library {
            MainView::Library
        } else {
            MainView::Queue
        };
        app.search_keyword = self.search_keyword.clone();
        app.search_page = self.search_page;
        app.search_pages = self.search_pages;
        app.search_total = self.search_total;
        app.lyrics_key.clear();
        app.lyrics.clear();
        app.lyrics_status.clear();
        // The cover of the paused track shows without replaying it: keep the
        // same `bvid#cid#page` key the replay would produce so the fetched
        // bytes match both before and after playback starts. Old files without
        // the 分P list fall back to cid 0 and simply refetch on replay.
        app.cover_key = app
            .now_playing
            .as_ref()
            .map(|item| {
                let cid = self.pages.get(self.page).map(|page| page.cid).unwrap_or(0);
                format!("{}#{}#{}", item.bvid, cid, self.page)
            })
            .unwrap_or_default();
        app.cache_progress = None;

        if app.now_playing.is_some() || !app.results.is_empty() {
            let count = app.results.len();
            let message = if count == 0 {
                "已恢复上次播放".to_string()
            } else {
                format!("已恢复上次播放 · {count} 首")
            };
            app.set_status(&message, false);
        }

        app.reset_shuffle();
        self.resume_request(app)
    }

    /// Request that continues the saved track from the saved position. The
    /// position falls back to zero when it would restart at the very end.
    pub fn resume_request(&self, app: &App) -> Option<PlayRequest> {
        let item = app.now_playing.clone().or_else(|| {
            app.current
                .and_then(|index| app.results.get(index))
                .cloned()
        })?;
        let mut start_at = self.position.max(0.0);
        if start_at < MIN_RESUME_SECS
            || (self.duration > 0.0 && start_at >= self.duration - END_GUARD_SECS)
        {
            start_at = 0.0;
        }
        Some(PlayRequest {
            item,
            page_index: app.page,
            start_at,
        })
    }
}

pub fn playback_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(PLAYBACK_FILE_NAME)
}

/// Load the saved session. Missing, unreadable or future-version files are
/// treated as "nothing to restore".
pub fn load(data_dir: &Path) -> Option<SavedPlayback> {
    let bytes = std::fs::read(playback_path(data_dir)).ok()?;
    let saved: SavedPlayback = serde_json::from_slice(&bytes).ok()?;
    (saved.version == VERSION).then_some(saved)
}

pub fn save(data_dir: &Path, saved: &SavedPlayback) -> Result<()> {
    let path = playback_path(data_dir);
    let bytes = serde_json::to_vec(saved).context("序列化播放进度失败")?;
    write_atomic(&path, &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("播放进度路径缺少父目录")?;
    std::fs::create_dir_all(parent).context("创建配置目录失败")?;
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp.{}.{suffix}", std::process::id()));
    std::fs::write(&tmp, bytes).context("写入播放进度临时文件失败")?;
    std::fs::rename(&tmp, path).context("提交播放进度失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(bvid: &str, title: &str) -> VideoItem {
        VideoItem {
            aid: 1,
            bvid: bvid.to_string(),
            title: title.to_string(),
            author: "UP".to_string(),
            duration: "04:00".to_string(),
            pic: None,
        }
    }

    fn roundtrip(app: &App) -> SavedPlayback {
        serde_json::from_slice(&serde_json::to_vec(&SavedPlayback::capture(app)).unwrap()).unwrap()
    }

    #[test]
    fn capture_and_apply_restore_queue_and_position() {
        let mut app = App::new();
        app.results = vec![item("BV1", "第一首"), item("BV2", "第二首")];
        app.results_title = "歌单 · 测试".to_string();
        app.current_playlist = Some(FavoriteFolder {
            id: 7,
            title: "测试".to_string(),
            count: 2,
        });
        app.current = Some(1);
        app.selected = Some(1);
        app.now_playing = Some(item("BV2", "第二首"));
        app.page = 0;
        app.position = 42.5;
        app.duration = 240.0;
        app.has_media = true;
        app.paused = false;
        app.search_keyword = "关键词".to_string();
        app.search_page = 2;
        app.search_pages = 5;

        let saved = roundtrip(&app);
        let mut restored = App::new();
        let request = saved.apply(&mut restored).expect("resume request");

        assert_eq!(restored.results.len(), 2);
        assert_eq!(restored.results_title, "歌单 · 测试");
        assert_eq!(restored.current, Some(1));
        assert_eq!(restored.selected, Some(1));
        assert_eq!(restored.current_playlist.as_ref().unwrap().id, 7);
        assert_eq!(restored.search_page, 2);
        assert!(!restored.has_media);
        assert!(restored.paused, "startup must not auto-play");
        assert_eq!(request.item.bvid, "BV2");
        assert_eq!(request.start_at, 42.5);
    }

    #[test]
    fn pages_survive_a_restore_so_the_picker_works_offline() {
        let mut app = App::new();
        app.results = vec![item("BV1", "合集")];
        app.current = Some(0);
        app.now_playing = Some(item("BV1", "合集"));
        app.page = 1;
        app.page_count = 2;
        app.playing_pages = vec![
            PageSummary {
                cid: 11,
                title: "上集".to_string(),
                duration: "4:00".to_string(),
            },
            PageSummary {
                cid: 12,
                title: "下集".to_string(),
                duration: "3:30".to_string(),
            },
        ];

        let saved = roundtrip(&app);
        let mut restored = App::new();
        saved.apply(&mut restored).expect("resume request");
        assert_eq!(restored.playing_pages.len(), 2);
        assert_eq!(restored.playing_pages[1].title, "下集");
        assert_eq!(restored.page, 1);
        // The cover key matches what replay would produce, so the restored
        // cover also stays valid after playback starts.
        assert_eq!(restored.cover_key, "BV1#12#1");

        // Without a track there is no video the pages could belong to.
        app.current = None;
        app.now_playing = None;
        let saved = roundtrip(&app);
        let mut restored = App::new();
        assert!(saved.apply(&mut restored).is_none());
        assert!(restored.playing_pages.is_empty());
        assert!(restored.cover_key.is_empty());
    }

    #[test]
    fn end_guard_restarts_instead_of_resuming_at_the_end() {
        let mut app = App::new();
        app.results = vec![item("BV1", "第一首")];
        app.now_playing = Some(item("BV1", "第一首"));
        app.current = Some(0);
        app.position = 238.0;
        app.duration = 240.0;
        app.paused = true;

        let saved = roundtrip(&app);
        let mut restored = App::new();
        let request = saved.apply(&mut restored).unwrap();
        assert_eq!(request.start_at, 0.0);
        assert!(restored.paused);
    }

    #[test]
    fn short_positions_and_bad_indices_are_safe() {
        // An out-of-range current index must not point into the restored
        // queue, and without a track there is nothing to resume.
        let mut app = App::new();
        app.results = vec![item("BV1", "第一首")];
        app.current = Some(9);
        app.now_playing = None;
        app.position = 1.0;

        let saved = roundtrip(&app);
        let mut restored = App::new();
        assert!(saved.apply(&mut restored).is_none());
        assert_eq!(restored.current, None);
        assert_eq!(restored.selected, Some(0));

        // A track paused in its first seconds starts over.
        app.current = Some(0);
        app.now_playing = Some(item("BV1", "第一首"));
        let saved = roundtrip(&app);
        let mut restored = App::new();
        let request = saved.apply(&mut restored).unwrap();
        assert_eq!(request.start_at, 0.0);
        assert!(restored.paused);
    }

    #[test]
    fn files_roundtrip_and_bad_versions_are_ignored() {
        let dir = std::env::temp_dir().join(format!(
            "bvox-resume-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut app = App::new();
        app.results = vec![item("BV1", "第一首")];
        app.current = Some(0);
        app.now_playing = Some(item("BV1", "第一首"));
        app.position = 12.0;

        let saved = SavedPlayback::capture(&app);
        save(&dir, &saved).unwrap();
        let loaded = load(&dir).expect("saved session");
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.position, 12.0);

        // A file written before the pages field existed still restores.
        let path = playback_path(&dir);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("pages");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load(&dir).is_some());

        // A file from a newer version must not be interpreted with old rules.
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["version"] = serde_json::json!(VERSION + 1);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load(&dir).is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
