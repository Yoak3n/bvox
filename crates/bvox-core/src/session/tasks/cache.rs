//! Background audio cache download task.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::{ApiClient, AudioStream};
use crate::session::msg::Msg;

pub(in crate::session) fn spawn_cache_download(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    key: String,
    bvid: String,
    cid: i64,
    stream: AudioStream,
) {
    if !api.audio_cache_enabled() {
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
