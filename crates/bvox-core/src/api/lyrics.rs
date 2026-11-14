//! Subtitle / lyrics fetching and parsing.

use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use super::sign::{encode_component, mixin_key, signed_query};
use super::text::normalize_url;
use super::{ApiClient, LyricLine};

impl ApiClient {
    async fn player_v2_wbi_url(&self, bvid: &str, cid: i64) -> Result<String> {
        let (img_key, sub_key) = self.wbi_keys().await?;
        let mixin = mixin_key(&img_key, &sub_key);
        let query = signed_query(
            vec![("bvid", bvid.to_string()), ("cid", cid.to_string())],
            &mixin,
        );
        Ok(format!("https://api.bilibili.com/x/player/wbi/v2?{query}"))
    }

    /// Fetch subtitle lines for a video. Returns an empty list when the video
    /// has no subtitles; network/parse failures are errors. Non-empty results
    /// are cached because a cid's subtitles do not change.
    ///
    /// `aid` is used alongside `cid` to verify AI subtitle URLs returned by
    /// the unsigned endpoint (see [`subtitle_belongs_to_video`]).
    pub async fn lyrics(&self, aid: i64, bvid: &str, cid: i64) -> Result<Vec<LyricLine>> {
        if let Some(lines) = self.cache.get_lyrics(bvid, cid) {
            return Ok(lines);
        }
        self.download_lyrics(aid, bvid, cid).await
    }

    /// Re-fetch lyrics for a page, discarding any cached copy first. Frontends
    /// use this when the displayed lyrics look wrong.
    pub async fn refresh_lyrics(&self, aid: i64, bvid: &str, cid: i64) -> Result<Vec<LyricLine>> {
        self.cache.remove_lyrics(bvid, cid);
        self.download_lyrics(aid, bvid, cid).await
    }

    async fn download_lyrics(&self, aid: i64, bvid: &str, cid: i64) -> Result<Vec<LyricLine>> {
        let lines = self.fetch_lyrics(aid, bvid, cid).await?;
        self.cache.put_lyrics(bvid, cid, &lines);
        Ok(lines)
    }

    async fn fetch_lyrics(&self, aid: i64, bvid: &str, cid: i64) -> Result<Vec<LyricLine>> {
        let plain_url = format!(
            "https://api.bilibili.com/x/player/v2?bvid={}&cid={cid}",
            encode_component(bvid)
        );

        let mut last_error = None;
        let mut saw_response = false;
        for attempt in 0..3 {
            // The unsigned endpoint intermittently answers with a different
            // video's subtitle list (an unrelated AI transcript cached under
            // this song). The WBI-signed endpoint is stable, so prefer it and
            // fall back only when signing itself fails.
            let (request_url, trusted) = match self.player_v2_wbi_url(bvid, cid).await {
                Ok(url) => (url, true),
                Err(error) => {
                    last_error = Some(error.context("字幕请求签名失败"));
                    (plain_url.clone(), false)
                }
            };

            let subtitles = match self.get_json::<PlayerV2Response>(&request_url, None).await {
                Ok(resp) if resp.code == 0 => {
                    saw_response = true;
                    resp.data
                        .and_then(|data| data.subtitle)
                        .and_then(|subtitle| subtitle.subtitles)
                        .unwrap_or_default()
                }
                Ok(resp) => {
                    last_error = Some(anyhow::anyhow!(
                        "获取字幕信息失败 {}: {}",
                        resp.code,
                        resp.message.unwrap_or_else(|| "未知错误".to_string())
                    ));
                    Vec::new()
                }
                Err(error) => {
                    last_error = Some(error.context("获取字幕信息失败"));
                    Vec::new()
                }
            };

            let candidates = subtitles
                .iter()
                .filter(|subtitle| {
                    subtitle
                        .subtitle_url
                        .as_deref()
                        .is_some_and(|url| !url.is_empty())
                        && (trusted || subtitle_belongs_to_video(aid, cid, subtitle))
                })
                .collect::<Vec<_>>();

            if !candidates.is_empty() {
                let chosen = choose_subtitle(&candidates).expect("字幕候选非空时必定能选出一条");
                let Some(subtitle_url) = chosen.subtitle_url.as_deref() else {
                    return Ok(Vec::new());
                };
                let body = self
                    .get_text(&normalize_url(subtitle_url), None)
                    .await
                    .context("下载字幕失败")?;
                return parse_lyrics(&body);
            }

            // Entries with empty URLs are usually rate limiting; retry. A
            // trusted empty list, on the other hand, really means this video
            // has no subtitles.
            if trusted && subtitles.is_empty() {
                break;
            }
            if attempt < 2 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        if saw_response {
            Ok(Vec::new())
        } else {
            Err(last_error.unwrap_or_else(|| anyhow::anyhow!("获取字幕信息失败")))
        }
    }
}
pub(super) fn parse_lyrics(body: &str) -> Result<Vec<LyricLine>> {
    let value: Value = serde_json::from_str(body).context("字幕 JSON 解析失败")?;
    let items = lyric_items(&value).context("字幕格式无法解析：找不到字幕行数组")?;

    let mut lines = Vec::new();
    for item in items {
        let Ok(raw) = serde_json::from_value::<RawLyric>(item.clone()) else {
            continue;
        };
        let text = raw.content.trim().to_string();
        if !text.is_empty() {
            lines.push(LyricLine {
                from: raw.from,
                to: raw.to,
                text,
            });
        }
    }
    Ok(lines)
}

/// Bilibili returns either a bare array or an object like
/// `{"type":"AIsubtitle","body":[...]}`.
pub(super) fn lyric_items(value: &Value) -> Option<&Vec<Value>> {
    match value {
        Value::Array(items) => Some(items),
        Value::Object(map) => ["body", "subtitles", "lines", "data", "result"]
            .iter()
            .find_map(|key| map.get(*key).and_then(lyric_items)),
        _ => None,
    }
}

/// Uploaded subtitles beat AI ones; within each group prefer Chinese. Uploaded
/// tracks are the closest thing to real lyrics, while AI transcripts can be
/// speech recognition of commentary instead of the song.
pub(super) fn choose_subtitle<'a>(candidates: &[&'a SubtitleRaw]) -> Option<&'a SubtitleRaw> {
    let is_ai = |subtitle: &&SubtitleRaw| subtitle.lan.starts_with("ai-");

    candidates
        .iter()
        .copied()
        .find(|subtitle| !is_ai(subtitle) && subtitle.lan.starts_with("zh"))
        .or_else(|| {
            candidates
                .iter()
                .copied()
                .find(|subtitle| subtitle.lan == "ai-zh")
        })
        .or_else(|| candidates.iter().copied().find(|subtitle| !is_ai(subtitle)))
        .or_else(|| candidates.first().copied())
}

/// AI subtitle files are named `<aid><cid><hash>` under `/bfs/ai_subtitle/`.
/// The unsigned `/x/player/v2` endpoint sometimes returns a well-formed URL
/// that belongs to a different video; comparing the file name against this
/// video's ids rejects those entries.
pub(super) fn subtitle_belongs_to_video(aid: i64, cid: i64, subtitle: &SubtitleRaw) -> bool {
    let Some(url) = subtitle.subtitle_url.as_deref() else {
        return false;
    };
    if !url.contains("/ai_subtitle/") {
        return true;
    }
    if aid <= 0 {
        return false;
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let file = path.rsplit('/').next().unwrap_or(path);
    file.starts_with(&format!("{aid}{cid}"))
}
#[derive(Deserialize)]
pub(super) struct PlayerV2Response {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<PlayerV2Data>,
}

#[derive(Deserialize)]
pub(super) struct PlayerV2Data {
    pub(super) subtitle: Option<SubtitleData>,
}

#[derive(Deserialize)]
pub(super) struct SubtitleData {
    pub(super) subtitles: Option<Vec<SubtitleRaw>>,
}

#[derive(Deserialize)]
pub(super) struct SubtitleRaw {
    #[serde(default)]
    pub(super) lan: String,
    pub(super) subtitle_url: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawLyric {
    #[serde(default, alias = "start")]
    pub(super) from: f64,
    #[serde(default, alias = "end")]
    pub(super) to: f64,
    #[serde(default, alias = "text", alias = "body")]
    pub(super) content: String,
}
