//! DASH audio stream resolution and download.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use tokio::io::AsyncWriteExt;

use super::playurl::{AudioTrack, PlayUrlResponse};
use super::quality::{is_premium_audio, preferred_kind, track_kind};
use super::sign::encode_component;
use super::text::snippet;
use super::{ApiClient, AudioQuality, AudioSource, AudioStream, REFERER, UA};

impl ApiClient {
    /// Fetch the DASH audio stream for a video. No signature is required for this endpoint.
    ///
    /// `preference` orders the candidates; premium tracks are preflighted with
    /// a ranged request because the CDN answers 403 when the account lacks the
    /// 大会员 entitlement, in which case the next candidate wins.
    pub async fn audio_stream(
        &self,
        bvid: &str,
        cid: i64,
        preference: AudioQuality,
        vip: bool,
    ) -> Result<AudioStream> {
        let url = format!(
            "https://api.bilibili.com/x/player/playurl?bvid={}&cid={cid}&fnval=4048&fnver=0&fourk=1",
            encode_component(bvid)
        );
        let resp: PlayUrlResponse = self
            .get_json(&url, None)
            .await
            .context("获取播放地址失败")?;

        if resp.code != 0 {
            bail!(
                "无法获取音频流 {}: {}{}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string()),
                match resp.code {
                    -404 => "（视频不存在或已下架）",
                    -10403 => "（地区限制或需要大会员）",
                    _ => "",
                }
            );
        }

        let dash = resp
            .data
            .and_then(|data| data.dash)
            .context("响应中没有 DASH 数据")?;
        let candidates = order_audio_candidates(
            dash.audio.unwrap_or_default(),
            dash.flac.and_then(|flac| flac.audio),
            dash.dolby.and_then(|dolby| dolby.audio).unwrap_or_default(),
            preference,
            vip,
        );

        let mut rejected_premium = false;
        for track in candidates {
            let Some(url) = track
                .base_url
                .or_else(|| track.backup_url.into_iter().next())
            else {
                continue;
            };
            if is_premium_audio(track.id) && !self.premium_stream_playable(&url).await {
                rejected_premium = true;
                continue;
            }
            return Ok(AudioStream {
                url,
                id: track.id,
                bandwidth: track.bandwidth,
            });
        }

        if rejected_premium {
            bail!("高音质流不可用（需要大会员权益），且没有可回退的 AAC 流");
        }
        bail!("该视频没有可用的音频流")
    }

    /// Premium DASH tracks answer 403 on the CDN when the account has no
    /// entitlement, even if `playurl` listed them. Probe with a 2-byte range
    /// request and let the caller fall back instead of handing the player a
    /// dead URL.
    async fn premium_stream_playable(&self, url: &str) -> bool {
        let request = self
            .http
            .get(url)
            .header("User-Agent", UA)
            .header("Referer", REFERER)
            .header("Range", "bytes=0-1")
            .timeout(Duration::from_secs(5));
        matches!(request.send().await, Ok(response) if response.status().is_success())
    }

    /// Prefer a finished cache file; fall back to a freshly resolved stream URL.
    pub async fn audio_source(
        &self,
        bvid: &str,
        cid: i64,
        preference: AudioQuality,
        vip: bool,
    ) -> Result<AudioSource> {
        let cached = self.cache.find_audio(bvid, cid);

        // A cached file already at the preference's top quality is final.
        if let Some(cached) = &cached
            && track_kind(cached.quality_id) == preferred_kind(preference, vip)
        {
            return Ok(AudioSource::Cached(cached.clone()));
        }

        match self.audio_stream(bvid, cid, preference, vip).await {
            Ok(stream) => {
                // The fresh stream is the same encoding as the cached copy;
                // keep using the local file.
                if let Some(cached) = cached
                    && cached.quality_id == stream.id
                {
                    return Ok(AudioSource::Cached(cached));
                }
                Ok(AudioSource::Remote(stream))
            }
            Err(error) => cached.map(AudioSource::Cached).ok_or(error),
        }
    }

    /// Download a stream into the audio cache, reporting `(downloaded, total)`
    /// as it goes. On any failure the partial file is removed and the error is
    /// returned; playback itself is unaffected by the caller.
    pub async fn download_audio<F>(
        &self,
        bvid: &str,
        cid: i64,
        stream: &AudioStream,
        on_progress: F,
    ) -> Result<()>
    where
        F: Fn(u64, Option<u64>) + Send + 'static,
    {
        if !self.cache.audio_enabled() {
            bail!("本地缓存已关闭");
        }

        let final_path = self
            .cache
            .audio_target(bvid, cid, stream.id, stream.bandwidth);
        let part_path = self.cache.audio_part(&final_path);
        if let Some(parent) = final_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("创建缓存目录失败")?;
        }

        let result = async {
            let mut response = self
                .http
                .get(&stream.url)
                .header("User-Agent", UA)
                .header("Referer", REFERER)
                // The client default is a 20s whole-request timeout, far too
                // short for a multi-megabyte audio file.
                .timeout(Duration::from_secs(600))
                .send()
                .await
                .context("下载音频失败")?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                bail!("下载音频 HTTP {status}: {}", snippet(&body));
            }

            let total = response.content_length();
            let step = progress_step(total);
            let mut file = tokio::fs::File::create(&part_path)
                .await
                .context("创建缓存文件失败")?;
            let mut downloaded = 0u64;
            let mut reported = 0u64;
            let mut last_emit = Instant::now();
            on_progress(0, total);

            while let Some(chunk) = response.chunk().await.context("读取音频流失败")? {
                file.write_all(&chunk).await.context("写入缓存失败")?;
                downloaded += chunk.len() as u64;
                if downloaded - reported >= step
                    && last_emit.elapsed() >= Duration::from_millis(200)
                {
                    reported = downloaded;
                    last_emit = Instant::now();
                    on_progress(downloaded, total);
                }
            }

            file.flush().await.context("写入缓存失败")?;
            drop(file);

            if let Some(total) = total
                && downloaded != total
            {
                bail!("音频下载不完整: {downloaded}/{total} 字节");
            }

            self.cache
                .commit_audio(&part_path, bvid, cid, &final_path)?;
            on_progress(downloaded, total);
            Ok(())
        }
        .await;

        if result.is_err() {
            let _ = tokio::fs::remove_file(&part_path).await;
        }
        result
    }
}
/// Add `track` to `candidates` unless an encoding with the same id is already
/// queued (some responses list Dolby both in `audio` and `dolby`).
pub(super) fn push_unique(candidates: &mut Vec<AudioTrack>, track: Option<AudioTrack>) {
    if let Some(track) = track
        && !candidates.iter().any(|existing| existing.id == track.id)
    {
        candidates.push(track);
    }
}

/// Order DASH candidates for a preference: the first entry is playback's first
/// choice, later entries are fallbacks (premium -> AAC when entitlement is
/// missing). Premium tracks are kept last for non-VIP `Auto` accounts so a
/// video that only has premium audio still has a chance to play.
pub(super) fn order_audio_candidates(
    aac: Vec<AudioTrack>,
    flac: Option<AudioTrack>,
    dolby: Vec<AudioTrack>,
    preference: AudioQuality,
    vip: bool,
) -> Vec<AudioTrack> {
    let best_aac = aac.into_iter().max_by_key(|stream| stream.bandwidth);
    let best_dolby = dolby.into_iter().max_by_key(|stream| stream.bandwidth);
    let mut candidates = Vec::new();

    match preference {
        AudioQuality::Auto if vip => {
            push_unique(&mut candidates, flac);
            push_unique(&mut candidates, best_dolby);
            push_unique(&mut candidates, best_aac);
        }
        AudioQuality::Auto | AudioQuality::Aac => {
            push_unique(&mut candidates, best_aac);
            push_unique(&mut candidates, flac);
            push_unique(&mut candidates, best_dolby);
        }
        AudioQuality::Dolby => {
            push_unique(&mut candidates, best_dolby);
            push_unique(&mut candidates, best_aac);
            push_unique(&mut candidates, flac);
        }
        AudioQuality::HiRes => {
            push_unique(&mut candidates, flac);
            push_unique(&mut candidates, best_aac);
            push_unique(&mut candidates, best_dolby);
        }
    }

    candidates
}

/// Report download progress roughly 20 times, clamped to sane chunk sizes.
pub(super) fn progress_step(total: Option<u64>) -> u64 {
    total
        .map(|total| (total / 20).clamp(64 * 1024, 1024 * 1024))
        .unwrap_or(256 * 1024)
}
