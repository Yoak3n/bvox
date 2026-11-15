//! Search commands.

use crate::app::MainView;
use crate::session::Session;
use crate::session::msg::Msg;
use crate::session::tasks::search::{spawn_search, spawn_search_more};

impl Session {
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
}
