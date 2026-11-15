//! Background search tasks.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::session::msg::Msg;

pub(in crate::session) fn spawn_search(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    seq: u64,
    keyword: String,
) {
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

pub(in crate::session) fn spawn_search_more(
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
