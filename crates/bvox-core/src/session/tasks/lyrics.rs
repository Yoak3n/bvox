//! Background lyrics task.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::session::msg::Msg;

pub(in crate::session) fn spawn_lyrics(
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
