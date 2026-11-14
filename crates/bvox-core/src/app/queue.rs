//! Queue stepping and selection.

use super::App;
use super::{MainView, PlayRequest};
use crate::api::{FavoriteFolder, PlayMode, VideoItem};

impl App {
    /// Move forward/backward by one track. A multi-page video is treated as
    /// several consecutive tracks: pages first, then the queue entry chosen
    /// by the current play mode.
    pub fn step(&mut self, delta: isize) -> Option<PlayRequest> {
        if self.results.is_empty() {
            return None;
        }

        if delta > 0 && self.page + 1 < self.page_count && self.now_playing.is_some() {
            return self.step_page(1);
        }
        if delta < 0 && self.page > 0 && self.now_playing.is_some() {
            return self.step_page(-1);
        }

        if self.play_mode == PlayMode::Shuffle {
            return self.step_shuffle(delta);
        }
        self.step_queue(delta)
    }

    pub(super) fn step_page(&mut self, delta: isize) -> Option<PlayRequest> {
        let item = self.now_playing.clone()?;
        self.page = (self.page as isize + delta).max(0) as usize;
        self.pending_play = true;
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: self.page,
            start_at: 0.0,
        })
    }

    pub(super) fn replay_current(&mut self) -> Option<PlayRequest> {
        let item = self.now_playing.clone().or_else(|| {
            self.current
                .and_then(|index| self.results.get(index))
                .cloned()
        })?;
        self.pending_play = true;
        self.set_status("单曲循环…", false);
        Some(PlayRequest {
            item,
            page_index: self.page,
            start_at: 0.0,
        })
    }

    pub(super) fn step_queue(&mut self, delta: isize) -> Option<PlayRequest> {
        let len = self.results.len();
        if len == 0 {
            return None;
        }
        let base = self.current.or(self.selected).unwrap_or(0) as isize;
        let next = base + delta;
        let index = if next < 0 || next as usize >= len {
            if self.play_mode == PlayMode::RepeatAll {
                next.rem_euclid(len as isize) as usize
            } else {
                self.set_status(
                    if delta > 0 {
                        "已经是最后一首"
                    } else {
                        "已经是第一首"
                    },
                    false,
                );
                return None;
            }
        } else {
            next as usize
        };
        self.start_queue_index(index, false)
    }
}
impl App {
    pub(super) fn start_queue_index(
        &mut self,
        index: usize,
        record_history: bool,
    ) -> Option<PlayRequest> {
        let item = self.results.get(index)?.clone();
        if record_history
            && self.play_mode == PlayMode::Shuffle
            && let Some(current) = self.current
            && current != index
        {
            self.shuffle_history.push(current);
        }
        self.current = Some(index);
        self.select(index);
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        self.playing_pages.clear();
        self.pending_play = true;
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: 0,
            start_at: 0.0,
        })
    }
}
impl App {
    pub fn play_selected(&mut self) -> Option<PlayRequest> {
        let index = self.selected?;
        self.play_at(index)
    }
}
impl App {
    /// Start playing the queue entry at `index` from its first page.
    pub fn play_at(&mut self, index: usize) -> Option<PlayRequest> {
        let item = self.results.get(index)?.clone();
        self.selected = Some(index);
        self.current = Some(index);
        self.play_failures = 0;
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        self.playing_pages.clear();
        self.pending_play = true;
        self.reset_shuffle();
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: 0,
            start_at: 0.0,
        })
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let current = self.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.results.len() as isize - 1) as usize;
        self.select(next);
    }

    /// Move the main-panel cursor according to the visible list.
    pub fn move_main_selection(&mut self, delta: isize) {
        match self.view {
            MainView::Library => self.move_playlist_selection(delta),
            MainView::Queue => self.move_selection(delta),
        }
    }

    pub(super) fn replace_results(
        &mut self,
        items: Vec<VideoItem>,
        title: &str,
        playlist: Option<FavoriteFolder>,
    ) {
        let empty = items.is_empty();
        self.results = items;
        self.results_title = title.to_string();
        self.current_playlist = playlist;
        self.play_failures = 0;
        self.current = None;
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        if empty {
            self.selected = None;
        } else {
            self.select(0);
        }
        self.clear_search_paging();
        self.reset_shuffle();
    }

    pub(super) fn select(&mut self, index: usize) {
        self.selected = Some(index);
    }
}
