//! Background favorite-folder tasks.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::{ApiClient, FavoriteFolder};
use crate::app::PendingImport;
use crate::session::msg::Msg;

pub(in crate::session) fn spawn_playlists(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    seq: u64,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let folders = api.favorite_folders().await?;
            let hidden = api.hidden_playlists().await;
            Ok::<_, anyhow::Error>((folders, hidden))
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistsLoaded { seq, result });
    });
}

pub(in crate::session) fn spawn_set_playlist_hidden(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    hidden: bool,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .set_playlist_hidden(folder_id, hidden)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistHidden {
            folder_id,
            hidden,
            result,
        });
    });
}

pub(in crate::session) fn spawn_open_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    seq: u64,
    folder: FavoriteFolder,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let (result, offline) = match api.favorite_items(folder.id).await {
            Ok(items) => (Ok(items), false),
            Err(error) => match api.cached_favorite_items(folder.id).await {
                Some(items) => (Ok(items), true),
                None => (Err(format!("{error:#}")), false),
            },
        };
        let _ = tx.send(Msg::PlaylistOpened {
            seq,
            folder,
            result,
            offline,
        });
    });
}

pub(in crate::session) fn spawn_create_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    title: String,
    pending: Option<PendingImport>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .create_playlist(&title)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistCreated { result, pending });
    });
}

pub(in crate::session) fn spawn_rename_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    title: String,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .rename_playlist(folder_id, &title)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistRenamed {
            folder_id,
            title,
            result,
        });
    });
}

pub(in crate::session) fn spawn_delete_playlist(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = api
            .delete_playlist(folder_id)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::PlaylistDeleted { folder_id, result });
    });
}
