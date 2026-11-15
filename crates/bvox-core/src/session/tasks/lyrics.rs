//! Background lyrics task.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::session::msg::Msg;

/// Everything one lyrics fetch needs: the display key, the track identity,
/// the sequence number and whether the disk cache should be bypassed.
pub(in crate::session) struct LyricsRequest {
    pub key: String,
    pub aid: i64,
    pub bvid: String,
    pub cid: i64,
    pub seq: u64,
    pub refresh: bool,
}

pub(in crate::session) fn spawn_lyrics(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    request: LyricsRequest,
) {
    let LyricsRequest {
        key,
        aid,
        bvid,
        cid,
        seq,
        refresh,
    } = request;
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
