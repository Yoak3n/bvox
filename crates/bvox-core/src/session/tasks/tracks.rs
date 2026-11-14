//! Background track tasks: add/remove favorites and BV imports.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc;

use crate::api::{ApiClient, FavoriteFolder, VideoItem, format_secs};
use crate::app::BvImport;
use crate::session::msg::Msg;
use crate::session::parse::{parse_bvid, share_link};

/// Resolve the aid without a `view` request whenever the item already has it.
pub(in crate::session) async fn resolve_aid(
    api: &ApiClient,
    item: Option<&VideoItem>,
    bvid: &str,
) -> Result<i64> {
    if let Some(item) = item
        && item.aid > 0
    {
        return Ok(item.aid);
    }
    let aid = api.view(bvid).await?.aid;
    if aid == 0 {
        bail!("视频信息缺少 aid");
    }
    Ok(aid)
}

pub(in crate::session) fn spawn_add_track(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    bvid: String,
    item: Option<VideoItem>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let aid = resolve_aid(&api, item.as_ref(), &bvid).await?;
            api.set_favorite(folder_id, aid, true).await
        }
        .await;
        let (changed, result) = match result {
            Ok(changed) => (changed, Ok(())),
            Err(error) => (false, Err(format!("{error:#}"))),
        };
        let _ = tx.send(Msg::TrackAdded {
            folder_id,
            changed,
            result,
        });
    });
}

pub(in crate::session) fn spawn_remove_track(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    folder_id: i64,
    bvid: String,
    item: Option<VideoItem>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = async {
            let aid = resolve_aid(&api, item.as_ref(), &bvid).await?;
            api.set_favorite(folder_id, aid, false).await
        }
        .await;
        let (changed, result) = match result {
            Ok(changed) => (changed, Ok(())),
            Err(error) => (false, Err(format!("{error:#}"))),
        };
        let _ = tx.send(Msg::TrackRemoved {
            folder_id,
            bvid,
            changed,
            result,
        });
    });
}

pub(in crate::session) fn spawn_import_bv(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    raw: String,
    playlist: Option<FavoriteFolder>,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let result = import_bv(&api, &raw, playlist)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::BvImported(result));
    });
}

pub(in crate::session) async fn import_bv(
    api: &ApiClient,
    raw: &str,
    playlist: Option<FavoriteFolder>,
) -> Result<BvImport> {
    let bvid = match parse_bvid(raw) {
        Some(bvid) => bvid,
        None => {
            // Full links carry a BV id; short links (`b23.tv/...`, and the
            // share text wrapping them) only expose it after the redirect.
            let link = share_link(raw).context("无法识别 BV 号或链接")?;
            let resolved = api
                .resolve_share_link(link)
                .await
                .context("解析短链接失败")?;
            parse_bvid(&resolved).with_context(|| format!("链接中未找到 BV 号: {resolved}"))?
        }
    };
    let detail = api.view(&bvid).await?;
    if detail.aid == 0 {
        bail!("视频信息缺少 aid，无法加入歌单");
    }

    let total_duration = detail.pages.iter().map(|page| page.duration).sum::<i64>();
    let duration = if total_duration > 0 {
        total_duration
    } else {
        detail.duration
    };
    let item = VideoItem {
        aid: detail.aid,
        bvid: detail.bvid.clone(),
        title: detail.title.clone(),
        author: detail.author.clone(),
        duration: format_secs(duration),
        pic: detail.pic.clone(),
    };

    let logged_in = api.has_cookie().await;
    // A playlist opened in the UI is the natural destination; without one the
    // track is parked so the frontend can prompt for a new playlist.
    let (message, error, playlist) = if let Some(folder) = playlist {
        match api.set_favorite(folder.id, detail.aid, true).await {
            Ok(changed) => {
                let message = if changed {
                    format!("已加入歌单「{}」", folder.title)
                } else {
                    format!("已在歌单「{}」中", folder.title)
                };
                (message, false, changed.then_some(folder))
            }
            Err(error) => (format!("加入歌单失败: {error:#}"), true, None),
        }
    } else if logged_in {
        ("等待选择歌单".to_string(), false, None)
    } else {
        ("未登录，仅加入本地列表".to_string(), false, None)
    };

    Ok(BvImport {
        item,
        message,
        error,
        playlist,
        logged_in,
    })
}
