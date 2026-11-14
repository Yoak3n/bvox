//! Background playback tasks: source resolution and streaming.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use crate::api::{
    ApiClient, AudioQuality, AudioSource, VideoDetail, VideoItem, VideoPage, clean_title,
    cover_thumbnail_url, format_secs, quality_label,
};
use crate::app::{PageSummary, PlayRequest, PlayingInfo};
use crate::player::{Player, PlayerConfig};
use crate::session::msg::Msg;
use crate::session::tasks::cache::spawn_cache_download;

/// One-line explanation of what a preference resolves to for this account.
pub(in crate::session) fn quality_status(quality: AudioQuality, vip: bool) -> String {
    let hint = match (quality, vip) {
        (AudioQuality::Auto, true) => "大会员：优先 Hi-Res/Dolby",
        (AudioQuality::Auto, false) => "最高 192K AAC",
        (AudioQuality::Aac, _) => "最高 192K AAC",
        (AudioQuality::Dolby, true) => "优先 Dolby，不可用时回退 AAC",
        (AudioQuality::Dolby, false) => "需要大会员，无权益时回退 AAC",
        (AudioQuality::HiRes, true) => "优先 Hi-Res FLAC，不可用时回退 AAC",
        (AudioQuality::HiRes, false) => "需要大会员，无权益时回退 AAC",
    };
    format!("音质偏好：{}（{hint}；切歌后生效）", quality.label())
}
/// Title a frontend should show for the loaded track. Multi-P videos play
/// one 分P per track and the part title is the actual song name, so it wins
/// over the collection-style video title.
pub(in crate::session) fn track_display_title(
    video_title: &str,
    part: &str,
    page_count: usize,
) -> String {
    if page_count > 1 && !part.is_empty() {
        clean_title(part)
    } else {
        clean_title(video_title)
    }
}

pub(in crate::session) fn spawn_play(
    api: &Arc<ApiClient>,
    player: Arc<dyn Player>,
    tx: mpsc::UnboundedSender<Msg>,
    request: PlayRequest,
    quality: AudioQuality,
    vip: bool,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let PlayRequest {
            item,
            page_index,
            start_at,
        } = request;
        let item_pic = item.pic.clone();
        let result: Result<(
            VideoDetail,
            AudioSource,
            usize,
            usize,
            String,
            i64,
            Vec<PageSummary>,
        )> = async {
            let detail = api.view(&item.bvid).await?;
            let pages = if detail.pages.is_empty() {
                vec![VideoPage {
                    cid: detail.cid,
                    duration: detail.duration,
                    part: String::new(),
                }]
            } else {
                detail.pages.clone()
            };
            let index = page_index.min(pages.len().saturating_sub(1));
            let page = pages.get(index).context("页码超出范围")?;
            let summaries = pages
                .iter()
                .enumerate()
                .map(|(position, page)| PageSummary {
                    cid: page.cid,
                    title: if page.part.is_empty() {
                        format!("P{}", position + 1)
                    } else {
                        page.part.clone()
                    },
                    duration: format_secs(page.duration),
                })
                .collect();

            let source = api
                .audio_source(&detail.bvid, page.cid, quality, vip)
                .await?;
            let target = source.target();
            let config = PlayerConfig {
                user_agent: crate::api::UA.to_string(),
                referer: crate::api::REFERER.to_string(),
                title: track_display_title(&detail.title, &page.part, pages.len()),
                artist: detail.author.clone(),
                cover_url: detail
                    .pic
                    .clone()
                    .or_else(|| item_pic.clone())
                    .as_deref()
                    .map(cover_thumbnail_url),
                start_position: start_at.max(0.0),
            };
            player.load(&target, &config).await?;
            Ok((
                detail,
                source,
                index,
                pages.len(),
                page.part.clone(),
                page.duration,
                summaries,
            ))
        }
        .await;

        match result {
            Ok((detail, source, page_index, page_count, part, page_duration, summaries)) => {
                let pic = detail.pic.clone().or(item_pic);
                let cid = detail
                    .pages
                    .get(page_index)
                    .map(|page| page.cid)
                    .unwrap_or(detail.cid);
                let bvid = detail.bvid.clone();
                let aid = detail.aid;
                let cached = source.is_cached();
                let quality = format!(
                    "{} · {} kbps",
                    quality_label(source.quality_id()),
                    source.bandwidth() / 1000
                );
                let playing = VideoItem {
                    aid,
                    bvid: detail.bvid,
                    title: track_display_title(&detail.title, &part, page_count),
                    author: detail.author,
                    duration: format_secs(page_duration),
                    pic: pic.clone(),
                };
                let key = format!("{bvid}#{cid}#{page_index}");
                let _ = tx.send(Msg::Playing(PlayingInfo {
                    item: playing,
                    quality,
                    page_index,
                    page_count,
                    part,
                    page_duration,
                    cid,
                    pic,
                    pages: summaries,
                    start_position: start_at.max(0.0),
                    cached,
                }));

                // A cache miss streams immediately; write a copy in the
                // background so the next play uses the local file.
                if let AudioSource::Remote(stream) = source {
                    spawn_cache_download(&api, tx.clone(), key, bvid, cid, stream);
                }
            }
            Err(error) => {
                let _ = tx.send(Msg::PlayFailed(format!("{error:#}")));
            }
        }
    });
}
