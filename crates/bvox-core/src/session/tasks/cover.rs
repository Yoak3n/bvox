//! Background cover download task.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::session::msg::Msg;

pub(in crate::session) fn spawn_cover(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    key: String,
    pic: Option<String>,
) {
    let Some(pic) = pic else {
        return;
    };
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .image_bytes(&pic)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::CoverReady { key, result });
    });
}
