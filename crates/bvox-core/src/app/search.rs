//! Search result state and paging.

use std::collections::HashSet;

use super::App;
use super::MainView;
use crate::api::{PlayMode, SearchPage};

impl App {
    pub fn on_search_done(
        &mut self,
        seq: u64,
        keyword: String,
        result: Result<SearchPage, String>,
    ) {
        if seq != self.search_seq {
            return;
        }
        self.loading = false;
        self.view = MainView::Queue;
        match result {
            Ok(page) if page.items.is_empty() => {
                let page_no = page.page.max(1);
                self.replace_results(Vec::new(), "搜索结果", None);
                self.set_search_paging(keyword, page_no, page_no, page.total);
                self.set_status("没有搜索到结果", true);
            }
            Ok(page) => {
                let count = page.items.len();
                self.replace_results(page.items, "搜索结果", None);
                self.set_search_paging(keyword, page.page, page.pages, page.total);
                // Platform-neutral: the TUI renders its own key hints.
                if self.has_more_results() {
                    self.set_status(&format!("找到 {count} 条结果，还有更多"), false);
                } else {
                    self.set_status(&format!("找到 {count} 条结果"), false);
                }
            }
            Err(error) => {
                self.set_status(&error, true);
            }
        }
    }

    /// Append the next search page. Stale pages (a newer search started) and
    /// duplicates (Bilibili sometimes repeats hits across pages) are dropped.
    pub fn on_search_more(&mut self, seq: u64, result: Result<SearchPage, String>) {
        if seq != self.search_seq {
            return;
        }
        self.loading_more = false;
        match result {
            Ok(page) => {
                let mut known = self
                    .results
                    .iter()
                    .map(|item| item.bvid.clone())
                    .collect::<HashSet<_>>();
                let empty = page.items.is_empty();
                let first_new = self.results.len();
                let mut added = 0;
                for item in page.items {
                    if known.insert(item.bvid.clone()) {
                        self.results.push(item);
                        added += 1;
                    }
                }
                // Keep shuffle mode covering the freshly loaded tracks without
                // restarting the current pass.
                if self.play_mode == PlayMode::Shuffle && first_new < self.results.len() {
                    let tail = self.shuffle_remaining.len();
                    self.shuffle_remaining.extend(first_new..self.results.len());
                    self.shuffle_rng
                        .shuffle(&mut self.shuffle_remaining[tail..]);
                }
                let page_no = page.page.max(1);
                self.search_page = page_no;
                // An empty page always ends the run, even if `numPages` lied.
                self.search_pages = if empty {
                    page_no
                } else {
                    page.pages.max(page_no)
                };
                self.search_total = page.total;
                if self.selected.is_none() && !self.results.is_empty() {
                    self.select(0);
                }
                if added == 0 && !self.has_more_results() {
                    self.set_status("没有更多结果了", false);
                } else {
                    self.set_status(&format!("已加载 {} 条结果", self.results.len()), false);
                }
            }
            Err(error) => self.set_status(&error, true),
        }
    }

    /// True when the current queue is a search with another page available.
    pub fn has_more_results(&self) -> bool {
        !self.search_keyword.is_empty()
            && self.current_playlist.is_none()
            && self.search_page < self.search_pages
    }

    pub(super) fn set_search_paging(&mut self, keyword: String, page: u32, pages: u32, total: u64) {
        let page_no = page.max(1);
        self.search_keyword = keyword;
        self.search_page = page_no;
        self.search_pages = pages.max(page_no);
        self.search_total = total;
        self.loading_more = false;
    }

    pub(super) fn clear_search_paging(&mut self) {
        self.search_keyword.clear();
        self.search_page = 0;
        self.search_pages = 0;
        self.search_total = 0;
        self.loading_more = false;
    }
}
