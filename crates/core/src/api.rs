//! Bilibili API: WBI search, QR login, favorites, subtitles, covers and
//! audio-only DASH streams.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::CorePaths;
use crate::cache::{Cache, CachedAudio};

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const REFERER: &str = "https://www.bilibili.com/";

/// Permutation table used to derive the WBI mixin key.
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

pub const CONFIG_DIR_NAME: &str = "bili-music";
const COOKIE_FILE_NAME: &str = "cookie";
const SYNC_FOLDER_FILE_NAME: &str = "sync_folder";
const DEFAULT_SYNC_FOLDER_TITLE: &str = "bili-music 云同步";

/// Prefer IPv4 and put a reachable address first. Some Bilibili CDN nodes
/// publish addresses that accept DNS but silently drop connections, and
/// reqwest does not perform Happy Eyeballs fallback by itself.
#[derive(Debug)]
struct Ipv4FirstResolver {
    cache: Arc<Mutex<HashMap<String, (Vec<SocketAddr>, Instant)>>>,
}

impl Ipv4FirstResolver {
    fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn forget(&self, host: &str) {
        self.cache.lock().await.remove(host);
    }
}

impl Resolve for Ipv4FirstResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let cache = self.cache.clone();
        Box::pin(async move {
            let host = name.as_str().to_string();

            {
                let cached = cache.lock().await;
                if let Some((addrs, fetched_at)) = cached.get(&host)
                    && fetched_at.elapsed() < Duration::from_secs(300)
                {
                    return Ok(Box::new(addrs.clone().into_iter()) as Addrs);
                }
            }

            let mut addrs = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?
                .collect::<Vec<SocketAddr>>();
            let ipv4 = addrs
                .iter()
                .copied()
                .filter(SocketAddr::is_ipv4)
                .collect::<Vec<_>>();
            if !ipv4.is_empty() {
                addrs = ipv4;
            }

            if addrs.len() > 1
                && let Some(reachable) = first_reachable(&host, &addrs).await
            {
                addrs.retain(|addr| *addr != reachable);
                addrs.insert(0, reachable);
            }

            cache
                .lock()
                .await
                .insert(host, (addrs.clone(), Instant::now()));
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// Some CDN nodes accept TCP but never finish TLS/HTTP. Probe with the same
/// reqwest/rustls stack the real request will use and return the first working
/// address.
async fn first_reachable(host: &str, addrs: &[SocketAddr]) -> Option<SocketAddr> {
    let mut tasks = tokio::task::JoinSet::new();
    for addr in addrs.iter().copied() {
        let host = host.to_string();
        tasks.spawn(async move {
            let pinned = SocketAddr::new(addr.ip(), 443);
            let reachable = async {
                let client = reqwest::Client::builder()
                    .resolve(&host, pinned)
                    .timeout(Duration::from_secs(2))
                    .build()
                    .ok()?;
                let url = format!("https://{host}/");
                client.head(url).send().await.ok().map(|_| ())
            }
            .await
            .is_some();
            (addr, reachable)
        });
    }

    while let Some(result) = tasks.join_next().await {
        if let Ok((addr, true)) = result {
            return Some(addr);
        }
    }
    None
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct VideoItem {
    pub bvid: String,
    pub title: String,
    pub author: String,
    pub duration: String,
    pub pic: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoPage {
    pub cid: i64,
    pub duration: i64,
    pub part: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoDetail {
    pub bvid: String,
    pub aid: i64,
    pub cid: i64,
    pub title: String,
    pub author: String,
    pub duration: i64,
    pub pages: Vec<VideoPage>,
    pub pic: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AudioStream {
    pub url: String,
    pub id: i64,
    pub bandwidth: i64,
}

/// Where the audio for one page comes from: a finished cache file or the
/// (expiring) DASH URL returned by `playurl`.
#[derive(Clone, Debug)]
pub enum AudioSource {
    Cached(CachedAudio),
    Remote(AudioStream),
}

impl AudioSource {
    pub fn quality_id(&self) -> i64 {
        match self {
            Self::Cached(cached) => cached.quality_id,
            Self::Remote(stream) => stream.id,
        }
    }

    pub fn bandwidth(&self) -> i64 {
        match self {
            Self::Cached(cached) => cached.bandwidth,
            Self::Remote(stream) => stream.bandwidth,
        }
    }

    pub fn is_cached(&self) -> bool {
        matches!(self, Self::Cached(_))
    }

    /// A local file path or an HTTP URL, ready to hand to mpv.
    pub fn target(&self) -> String {
        match self {
            Self::Cached(cached) => cached.path.to_string_lossy().into_owned(),
            Self::Remote(stream) => stream.url.clone(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FavoriteFolder {
    pub id: i64,
    pub title: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, uniffi::Record)]
pub struct LyricLine {
    pub from: f64,
    /// Reserved for karaoke-style highlighting; the UI currently keys off `from`.
    #[allow(dead_code)]
    pub to: f64,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct UserInfo {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct QrLogin {
    pub url: String,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrPhase {
    Waiting,
    Scanned,
    Expired,
    Success,
}

pub struct ApiClient {
    paths: CorePaths,
    http: reqwest::Client,
    resolver: Arc<Ipv4FirstResolver>,
    cache: Cache,
    keys: Mutex<Option<(String, String, Instant)>>,
    buvid: Mutex<Option<String>>,
    cookie: Mutex<Option<String>>,
    sync_folder: Mutex<Option<i64>>,
}

impl ApiClient {
    pub fn new(paths: CorePaths) -> Result<Self> {
        let cookie = std::env::var("BILI_COOKIE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| load_saved_cookie(&paths.data_dir));

        let resolver = Arc::new(Ipv4FirstResolver::new());
        let http = reqwest::Client::builder()
            .dns_resolver(resolver.clone())
            .timeout(Duration::from_secs(20))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let cache_mb = paths.cache_mb;
        let cache_dir = paths.cache_dir.clone();
        let data_dir = paths.data_dir.clone();
        Ok(Self {
            http,
            resolver,
            paths,
            cache: Cache::new(cache_dir, cache_mb),
            keys: Mutex::new(None),
            buvid: Mutex::new(None),
            cookie: Mutex::new(cookie.clone()),
            sync_folder: Mutex::new(load_sync_folder_id(&data_dir)),
        })
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    pub fn cache_enabled(&self) -> bool {
        self.cache.enabled()
    }

    pub async fn has_cookie(&self) -> bool {
        self.cookie
            .lock()
            .await
            .as_ref()
            .is_some_and(|cookie| cookie.contains("SESSDATA="))
    }

    pub async fn set_cookie(&self, cookie: String) {
        *self.cookie.lock().await = Some(cookie.clone());
        let _ = save_cookie(&self.paths.data_dir, &cookie);
    }

    pub async fn user_info(&self) -> Result<Option<UserInfo>> {
        let resp: NavResponse = self
            .get_json("https://api.bilibili.com/x/web-interface/nav", None)
            .await?;
        if resp.code != 0 {
            return Ok(None);
        }
        let Some(data) = resp.data else {
            return Ok(None);
        };
        if !data.is_login {
            return Ok(None);
        }
        Ok(Some(UserInfo {
            name: data.uname.unwrap_or_default(),
        }))
    }

    pub async fn qr_generate(&self) -> Result<QrLogin> {
        let resp: QrGenerateResponse = self
            .get_json(
                "https://passport.bilibili.com/x/passport-login/web/qrcode/generate",
                None,
            )
            .await
            .context("获取登录二维码失败")?;

        if resp.code != 0 {
            bail!(
                "获取登录二维码失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let data = resp.data.context("二维码响应为空")?;
        Ok(QrLogin {
            url: data.url,
            key: data.qrcode_key,
        })
    }

    /// Poll the QR code once. On success the Cookie is persisted and installed
    /// into this client before returning.
    pub async fn qr_poll(&self, key: &str) -> Result<QrPhase> {
        let url = format!(
            "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}&source=main-fe-header",
            encode_component(key)
        );
        let mut request = self
            .http
            .get(url)
            .header("User-Agent", UA)
            .header("Referer", REFERER);
        if let Some(cookie) = self.combine_cookie(None).await {
            request = request.header("Cookie", cookie);
        }

        let response = request.send().await.context("轮询扫码状态失败")?;
        let set_cookies = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let body = response.text().await?;
        let resp: QrPollResponse = serde_json::from_str(&body)
            .with_context(|| format!("扫码响应不是 JSON: {}", snippet(&body)))?;

        if resp.code != 0 {
            bail!(
                "扫码失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let data = resp.data.context("扫码响应为空")?;
        match data.code {
            0 => {
                let cookie = cookie_from_set_cookies(&set_cookies)
                    .or_else(|| data.url.as_deref().and_then(cookie_from_url))
                    .context("登录成功但没有获取到 Cookie")?;
                self.set_cookie(cookie).await;
                Ok(QrPhase::Success)
            }
            86101 => Ok(QrPhase::Waiting),
            86090 => Ok(QrPhase::Scanned),
            86038 => Ok(QrPhase::Expired),
            other => bail!(
                "扫码状态异常 {other}: {}",
                data.message.unwrap_or_else(|| "未知错误".to_string())
            ),
        }
    }

    /// Search videos. The wbi endpoint requires a `buvid3` cookie and a signed query.
    pub async fn search(&self, keyword: &str) -> Result<Vec<VideoItem>> {
        let (img_key, sub_key) = self.wbi_keys().await?;
        let mixin_key = mixin_key(&img_key, &sub_key);
        let buvid = self.buvid_extra().await?;

        let query = signed_query(
            vec![
                ("search_type", "video".to_string()),
                ("keyword", keyword.to_string()),
                ("page", "1".to_string()),
            ],
            &mixin_key,
        );
        let url = format!("https://api.bilibili.com/x/web-interface/wbi/search/type?{query}");

        let resp: SearchResponse = self
            .get_json(&url, buvid.as_deref())
            .await
            .context("搜索请求失败")?;

        if resp.code != 0 {
            bail!(
                "搜索失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let items = resp
            .data
            .and_then(|data| data.result)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|raw| {
                let bvid = raw.bvid?;
                if bvid.is_empty() {
                    return None;
                }
                Some(VideoItem {
                    bvid,
                    title: clean_title(raw.title.as_deref().unwrap_or("")),
                    author: raw.author.unwrap_or_default(),
                    duration: normalize_duration(raw.duration.as_deref().unwrap_or("")),
                    pic: raw.pic.as_deref().map(normalize_url),
                })
            })
            .collect();

        Ok(items)
    }

    /// Resolve a BV id into its first page cid and metadata.
    ///
    /// Fresh cache entries avoid the request entirely. If the request fails
    /// (offline, risk control) a stale entry is still usable, which keeps
    /// already-cached songs playable.
    pub async fn view(&self, bvid: &str) -> Result<VideoDetail> {
        if let Some(detail) = self.cache.get_view(bvid) {
            return Ok(detail);
        }

        match self.fetch_view(bvid).await {
            Ok(detail) => {
                self.cache.put_view(bvid, &detail);
                Ok(detail)
            }
            Err(error) => match self.cache.get_stale_view(bvid) {
                Some(detail) => Ok(detail),
                None => Err(error),
            },
        }
    }

    async fn fetch_view(&self, bvid: &str) -> Result<VideoDetail> {
        let url = format!(
            "https://api.bilibili.com/x/web-interface/view?bvid={}",
            encode_component(bvid)
        );
        let resp: ViewResponse = self
            .get_json(&url, None)
            .await
            .context("获取视频信息失败")?;

        if resp.code != 0 {
            bail!(
                "获取视频信息失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        let data = resp.data.context("视频信息为空")?;
        let pages = data
            .pages
            .into_iter()
            .map(|page| VideoPage {
                cid: page.cid,
                duration: page.duration,
                part: page.part,
            })
            .collect::<Vec<_>>();

        // For multi-page videos the top-level cid/duration describe the first
        // page, but duration is the sum of all pages. Prefer the page values.
        let cid = pages.first().map(|page| page.cid).unwrap_or(data.cid);
        let duration = pages
            .first()
            .map(|page| page.duration)
            .filter(|duration| *duration > 0)
            .unwrap_or(data.duration);

        Ok(VideoDetail {
            bvid: data.bvid,
            aid: data.aid,
            cid,
            title: data.title,
            author: data.owner.name,
            duration,
            pages,
            pic: data.pic.as_deref().map(normalize_url),
        })
    }

    /// Fetch the DASH audio stream for a video. No signature is required for this endpoint.
    pub async fn audio_stream(&self, bvid: &str, cid: i64) -> Result<AudioStream> {
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
        let mut streams = dash.audio.unwrap_or_default();

        // Only fall back to premium tracks when there is no regular AAC stream.
        if streams.is_empty() {
            if let Some(flac) = dash.flac.and_then(|flac| flac.audio) {
                streams.push(flac);
            }
        }
        if streams.is_empty() {
            if let Some(dolby) = dash.dolby.and_then(|dolby| dolby.audio) {
                streams.extend(dolby);
            }
        }

        let best = streams
            .into_iter()
            .max_by_key(|stream| stream.bandwidth)
            .context("该视频没有可用的音频流")?;

        let url = best
            .base_url
            .or_else(|| best.backup_url.into_iter().next())
            .context("音频流缺少地址")?;

        Ok(AudioStream {
            url,
            id: best.id,
            bandwidth: best.bandwidth,
        })
    }

    /// Prefer a finished cache file; fall back to a freshly resolved stream URL.
    pub async fn audio_source(&self, bvid: &str, cid: i64) -> Result<AudioSource> {
        if let Some(cached) = self.cache.find_audio(bvid, cid) {
            return Ok(AudioSource::Cached(cached));
        }
        Ok(AudioSource::Remote(self.audio_stream(bvid, cid).await?))
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
        if !self.cache.enabled() {
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

    pub async fn favorite_folders(&self) -> Result<Vec<FavoriteFolder>> {
        let mid = self.mid().await.context("未登录，请先按 g 扫码登录")?;
        let url = format!("https://api.bilibili.com/x/v3/fav/folder/created/list-all?up_mid={mid}");
        let resp: FavoriteFoldersResponse =
            self.get_json(&url, None).await.context("获取收藏夹失败")?;

        if resp.code != 0 {
            bail!(
                "获取收藏夹失败 {}: {}",
                resp.code,
                resp.message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        Ok(resp
            .data
            .and_then(|data| data.list)
            .unwrap_or_default()
            .into_iter()
            .map(|folder| FavoriteFolder {
                id: folder.id,
                title: clean_title(&folder.title),
                count: folder.media_count,
            })
            .collect())
    }

    /// Import every playable video from a favorite folder (up to 100 pages).
    pub async fn favorite_items(&self, media_id: i64) -> Result<Vec<VideoItem>> {
        let mut items = Vec::new();

        for page in 1..=100 {
            let url = format!(
                "https://api.bilibili.com/x/v3/fav/resource/list?media_id={media_id}&pn={page}&ps=20&platform=web"
            );
            let resp: FavoriteItemsResponse = self
                .get_json(&url, None)
                .await
                .context("获取收藏夹内容失败")?;

            if resp.code != 0 {
                bail!(
                    "获取收藏夹内容失败 {}: {}",
                    resp.code,
                    resp.message.unwrap_or_else(|| "未知错误".to_string())
                );
            }

            let Some(data) = resp.data else {
                break;
            };
            let medias = data.medias.unwrap_or_default();
            if medias.is_empty() {
                break;
            }

            for media in medias {
                if media.media_type != 2 {
                    continue; // only regular videos are playable
                }
                let Some(bvid) = media.bvid.filter(|bvid| !bvid.is_empty()) else {
                    continue;
                };
                items.push(VideoItem {
                    bvid,
                    title: clean_title(&media.title),
                    author: media.upper.map(|upper| upper.name).unwrap_or_default(),
                    duration: format_secs(media.duration),
                    pic: media.cover.as_deref().map(normalize_url),
                });
            }

            if !data.has_more {
                break;
            }
        }

        Ok(items)
    }

    /// Return the dedicated private sync folder. With `create=false` this only
    /// looks it up by title in the current account; with `create=true` a stale
    /// cache or a missing folder is replaced/created.
    pub async fn sync_folder(&self, create: bool) -> Result<Option<FavoriteFolder>> {
        if create && let Some(id) = *self.sync_folder.lock().await {
            return Ok(Some(FavoriteFolder {
                id,
                title: sync_folder_title(),
                count: 0,
            }));
        }

        let folders = self.favorite_folders().await?;
        if let Some(folder) = folders
            .into_iter()
            .find(|folder| folder.title == sync_folder_title())
        {
            self.remember_sync_folder(folder.id).await;
            return Ok(Some(folder));
        }

        if !create {
            return Ok(None);
        }

        let id = self
            .create_favorite_folder(&sync_folder_title(), true)
            .await?;
        self.remember_sync_folder(id).await;
        Ok(Some(FavoriteFolder {
            id,
            title: sync_folder_title(),
            count: 0,
        }))
    }

    pub async fn sync_items(
        &self,
        create: bool,
    ) -> Result<Option<(FavoriteFolder, Vec<VideoItem>)>> {
        let Some(folder) = self.sync_folder(create).await? else {
            return Ok(None);
        };

        match self.favorite_items(folder.id).await {
            Ok(items) => Ok(Some((folder, items))),
            Err(_) if !create => Ok(None),
            Err(_) => {
                // The cached folder id may be stale (deleted on the web).
                self.forget_sync_folder().await;
                let Some(folder) = self.sync_folder(true).await? else {
                    return Ok(None);
                };
                let items = self.favorite_items(folder.id).await?;
                Ok(Some((folder, items)))
            }
        }
    }

    /// Add one video to the sync folder, creating the folder on first use.
    pub async fn add_to_sync(&self, aid: i64) -> Result<FavoriteFolder> {
        let folder = self
            .sync_folder(true)
            .await?
            .context("无法创建云同步收藏夹")?;

        match self.add_video_to_favorite(folder.id, aid).await {
            Ok(()) => Ok(folder),
            Err(first_error) => {
                // The cached folder may have been deleted on the web.
                self.forget_sync_folder().await;
                let folder = self
                    .sync_folder(true)
                    .await?
                    .context("无法创建云同步收藏夹")?;
                self.add_video_to_favorite(folder.id, aid)
                    .await
                    .with_context(|| format!("首次加入失败: {first_error:#}"))?;
                Ok(folder)
            }
        }
    }

    async fn create_favorite_folder(&self, title: &str, private: bool) -> Result<i64> {
        let csrf = self.csrf_token().await?;
        let body = format!(
            "title={}&intro={}&privacy={}&csrf={}",
            encode_component(title),
            encode_component("bili-music 云同步收藏夹"),
            if private { "1" } else { "0" },
            encode_component(&csrf)
        );
        let response = self
            .post_form("https://api.bilibili.com/x/v3/fav/folder/add", body)
            .await
            .context("创建收藏夹失败")?;

        let code = response.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if code != 0 {
            bail!(
                "创建收藏夹失败 {code}: {}",
                response
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("未知错误")
            );
        }

        if let Some(id) = response
            .get("data")
            .and_then(|data| data.get("id"))
            .and_then(Value::as_i64)
        {
            return Ok(id);
        }

        self.favorite_folders()
            .await?
            .into_iter()
            .find(|folder| folder.title == title)
            .map(|folder| folder.id)
            .context("收藏夹已创建但无法获取 id")
    }

    async fn add_video_to_favorite(&self, folder_id: i64, aid: i64) -> Result<()> {
        let csrf = self.csrf_token().await?;
        let body = format!(
            "rid={aid}&type=2&add_media_ids={folder_id}&del_media_ids=&csrf={}",
            encode_component(&csrf)
        );
        let response = self
            .post_form("https://api.bilibili.com/x/v3/fav/resource/deal", body)
            .await
            .context("加入收藏夹失败")?;

        let code = response.get("code").and_then(Value::as_i64).unwrap_or(-1);
        // 11201 means the video is already in this folder.
        if code == 0 || code == 11201 {
            return Ok(());
        }
        bail!(
            "加入收藏夹失败 {code}: {}",
            response
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        )
    }

    async fn remember_sync_folder(&self, id: i64) {
        *self.sync_folder.lock().await = Some(id);
        let _ = save_sync_folder_id(&self.paths.data_dir, id);
    }

    async fn forget_sync_folder(&self) {
        *self.sync_folder.lock().await = None;
        let _ = std::fs::remove_file(sync_folder_path(&self.paths.data_dir));
    }

    async fn csrf_token(&self) -> Result<String> {
        let cookie = self
            .cookie
            .lock()
            .await
            .clone()
            .context("未登录，请先按 g 扫码登录")?;
        cookie_field(&cookie, "bili_jct")
            .filter(|value| !value.is_empty())
            .context("Cookie 缺少 bili_jct，请重新扫码登录")
    }

    async fn post_form(&self, url: &str, body: String) -> Result<Value> {
        let mut request = self
            .http
            .post(url)
            .header("User-Agent", UA)
            .header("Referer", REFERER)
            .header("Origin", "https://www.bilibili.com")
            .header("Content-Type", "application/x-www-form-urlencoded");

        if let Some(cookie) = self.combine_cookie(None).await {
            request = request.header("Cookie", cookie);
        }

        let response = request.body(body).send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            bail!("HTTP {status}: {}", snippet(&text));
        }
        serde_json::from_str(&text).with_context(|| format!("响应不是 JSON: {}", snippet(&text)))
    }

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
    pub async fn lyrics(&self, bvid: &str, cid: i64) -> Result<Vec<LyricLine>> {
        if let Some(lines) = self.cache.get_lyrics(bvid, cid) {
            return Ok(lines);
        }
        let lines = self.fetch_lyrics(bvid, cid).await?;
        self.cache.put_lyrics(bvid, cid, &lines);
        Ok(lines)
    }

    async fn fetch_lyrics(&self, bvid: &str, cid: i64) -> Result<Vec<LyricLine>> {
        let url = format!(
            "https://api.bilibili.com/x/player/v2?bvid={}&cid={cid}",
            encode_component(bvid)
        );

        let mut subtitles = Vec::new();
        for attempt in 0..2 {
            let request_url = if attempt == 0 {
                url.clone()
            } else {
                self.player_v2_wbi_url(bvid, cid)
                    .await
                    .unwrap_or_else(|_| url.clone())
            };
            let resp: PlayerV2Response = self
                .get_json(&request_url, None)
                .await
                .context("获取字幕信息失败")?;

            if resp.code != 0 {
                bail!(
                    "获取字幕信息失败 {}: {}",
                    resp.code,
                    resp.message.unwrap_or_else(|| "未知错误".to_string())
                );
            }

            subtitles = resp
                .data
                .and_then(|data| data.subtitle)
                .and_then(|subtitle| subtitle.subtitles)
                .unwrap_or_default();

            // Bilibili sometimes returns subtitle entries with an empty URL
            // (especially under rate limiting). Retry once before giving up.
            let has_url = subtitles.iter().any(|subtitle| {
                subtitle
                    .subtitle_url
                    .as_deref()
                    .is_some_and(|url| !url.is_empty())
            });
            if has_url || subtitles.is_empty() {
                break;
            }
            if attempt == 0 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        let candidates = subtitles
            .iter()
            .filter(|subtitle| {
                subtitle
                    .subtitle_url
                    .as_deref()
                    .is_some_and(|url| !url.is_empty())
            })
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let chosen = candidates
            .iter()
            .copied()
            .find(|subtitle| subtitle.lan == "zh-CN" || subtitle.lan == "ai-zh")
            .or_else(|| {
                candidates
                    .iter()
                    .copied()
                    .find(|subtitle| subtitle.lan.starts_with("zh"))
            })
            .or_else(|| candidates.first().copied());

        let Some(subtitle) = chosen else {
            return Ok(Vec::new());
        };
        let Some(subtitle_url) = subtitle.subtitle_url.as_deref() else {
            return Ok(Vec::new());
        };

        let body = self
            .get_text(&normalize_url(subtitle_url), None)
            .await
            .context("下载字幕失败")?;
        parse_lyrics(&body)
    }

    pub async fn image_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let url = cover_thumbnail_url(url);
        if let Some(bytes) = self.cache.get_cover(&url) {
            return Ok(bytes);
        }

        let host = reqwest::Url::parse(&url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string));
        let mut last_error = None;

        for attempt in 0..2 {
            match self.try_image_bytes(&url).await {
                Ok(bytes) => {
                    self.cache.put_cover(&url, &bytes);
                    return Ok(bytes);
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt == 0 {
                        if let Some(host) = &host {
                            self.resolver.forget(host).await;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("下载封面失败")))
    }

    async fn try_image_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .http
            .get(url)
            .header("User-Agent", UA)
            .header("Referer", REFERER)
            .send()
            .await
            .context("下载封面失败")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("封面 HTTP {status}: {}", snippet(&body));
        }
        Ok(response.bytes().await?.to_vec())
    }

    async fn mid(&self) -> Option<i64> {
        let cookie = self.cookie.lock().await.clone()?;
        cookie_field(&cookie, "DedeUserID").and_then(|value| value.parse::<i64>().ok())
    }

    async fn wbi_keys(&self) -> Result<(String, String)> {
        {
            let guard = self.keys.lock().await;
            if let Some((img_key, sub_key, fetched_at)) = guard.as_ref()
                && fetched_at.elapsed() < Duration::from_secs(3600)
            {
                return Ok((img_key.clone(), sub_key.clone()));
            }
        }

        let resp: NavResponse = self
            .get_json("https://api.bilibili.com/x/web-interface/nav", None)
            .await
            .context("获取 WBI 密钥失败")?;
        // Even when logged out (code -101) the nav response still carries wbi_img.
        let wbi_img = resp
            .data
            .and_then(|data| data.wbi_img)
            .context("nav 响应缺少 wbi_img")?;
        let img_key = extract_key(&wbi_img.img_url).context("img_url 格式无效")?;
        let sub_key = extract_key(&wbi_img.sub_url).context("sub_url 格式无效")?;

        *self.keys.lock().await = Some((img_key.clone(), sub_key.clone(), Instant::now()));
        Ok((img_key, sub_key))
    }

    /// Bilibili's risk control wants a stable `buvid3` cookie on search requests.
    async fn buvid_extra(&self) -> Result<Option<String>> {
        if let Some(cookie) = self.cookie.lock().await.as_ref()
            && cookie.contains("buvid3=")
        {
            return Ok(None);
        }

        if let Some(cached) = self.buvid.lock().await.as_ref() {
            return Ok(Some(cached.clone()));
        }

        let resp: SpiResponse = self
            .get_json("https://api.bilibili.com/x/frontend/finger/spi", None)
            .await
            .context("获取 buvid3 失败")?;

        let mut cookie = format!("buvid3={}", resp.data.b_3);
        if let Some(buvid4) = resp.data.b_4 {
            cookie.push_str(&format!("; buvid4={buvid4}"));
        }
        *self.buvid.lock().await = Some(cookie.clone());
        Ok(Some(cookie))
    }

    async fn get_text(&self, url: &str, extra_cookie: Option<&str>) -> Result<String> {
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string));
        let mut last_error = None;

        for attempt in 0..2 {
            match self.try_get_text(url, extra_cookie).await {
                Ok(body) => return Ok(body),
                Err(error) => {
                    last_error = Some(error);
                    if attempt == 0 {
                        if let Some(host) = &host {
                            self.resolver.forget(host).await;
                        }
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("请求失败")))
    }

    async fn try_get_text(&self, url: &str, extra_cookie: Option<&str>) -> Result<String> {
        let mut request = self
            .http
            .get(url)
            .header("User-Agent", UA)
            .header("Referer", REFERER);

        if let Some(cookie) = self.combine_cookie(extra_cookie).await {
            request = request.header("Cookie", cookie);
        }

        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            bail!("HTTP {status}: {}", snippet(&body));
        }

        Ok(body)
    }

    async fn get_json<T>(&self, url: &str, extra_cookie: Option<&str>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let body = self.get_text(url, extra_cookie).await?;
        serde_json::from_str(&body)
            .with_context(|| format!("响应不是 JSON（可能被风控）: {}", snippet(&body)))
    }

    async fn combine_cookie(&self, extra: Option<&str>) -> Option<String> {
        let stored = self.cookie.lock().await.clone();
        match (stored, extra) {
            (Some(base), Some(extra)) => Some(format!("{base}; {extra}")),
            (Some(base), None) => Some(base),
            (None, Some(extra)) => Some(extra.to_string()),
            (None, None) => None,
        }
    }
}

pub fn quality_label(id: i64) -> &'static str {
    match id {
        30216 => "64K",
        30232 => "132K",
        30280 => "192K",
        30250 => "Dolby",
        30251 => "Hi-Res",
        _ => "AAC",
    }
}

pub fn format_secs(total: i64) -> String {
    let total = total.max(0);
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Report download progress roughly 20 times, clamped to sane chunk sizes.
fn progress_step(total: Option<u64>) -> u64 {
    total
        .map(|total| (total / 20).clamp(64 * 1024, 1024 * 1024))
        .unwrap_or(256 * 1024)
}

fn parse_lyrics(body: &str) -> Result<Vec<LyricLine>> {
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
fn lyric_items(value: &Value) -> Option<&Vec<Value>> {
    match value {
        Value::Array(items) => Some(items),
        Value::Object(map) => ["body", "subtitles", "lines", "data", "result"]
            .iter()
            .find_map(|key| map.get(*key).and_then(lyric_items)),
        _ => None,
    }
}

/// Search results use unpadded durations like `419:2`; normalize them to `6:59:02`.
fn normalize_duration(raw: &str) -> String {
    let parts = raw.split(':').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 {
        return raw.to_string();
    }

    let mut total = 0i64;
    for part in parts {
        let Ok(value) = part.parse::<i64>() else {
            return raw.to_string();
        };
        total = total * 60 + value;
    }
    format_secs(total)
}

fn signed_query(mut params: Vec<(&str, String)>, mixin_key: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    params.push(("wts", timestamp.to_string()));
    params.sort_by(|left, right| left.0.cmp(right.0));

    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value)))
        .collect::<Vec<_>>()
        .join("&");
    let w_rid = format!("{:x}", md5::compute(format!("{query}{mixin_key}")));

    format!("{query}&w_rid={w_rid}")
}

fn mixin_key(img_key: &str, sub_key: &str) -> String {
    let source = format!("{img_key}{sub_key}");
    let bytes = source.as_bytes();
    MIXIN_KEY_ENC_TAB
        .iter()
        .take(32)
        .filter_map(|&index| bytes.get(index).map(|byte| *byte as char))
        .collect()
}

fn extract_key(url: &str) -> Option<String> {
    url.rsplit('/')
        .next()
        .and_then(|file_name| file_name.split('.').next())
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
}

/// Percent-encode everything except RFC 3986 unreserved characters.
/// `!'()*` are intentionally encoded as well, matching Bilibili's WBI examples.
fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn clean_title(title: &str) -> String {
    let mut without_tags = String::with_capacity(title.len());
    let mut inside_tag = false;
    for character in title.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => without_tags.push(character),
            _ => {}
        }
    }
    without_tags
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn normalize_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

/// Ask Bilibili's image service for a small thumbnail instead of the original
/// 4K cover. The 32x16 terminal grid needs nothing larger.
fn cover_thumbnail_url(url: &str) -> String {
    let url = normalize_url(url);
    if !url.contains("hdslb.com") || url.contains('@') {
        return url;
    }
    if let Some((base, query)) = url.split_once('?') {
        format!("{base}@640w_360h.jpg?{query}")
    } else {
        format!("{url}@640w_360h.jpg")
    }
}

fn snippet(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 160 {
        format!("{}…", compact.chars().take(160).collect::<String>())
    } else {
        compact
    }
}

fn cookie_path(data_dir: &Path) -> PathBuf {
    data_dir.join(COOKIE_FILE_NAME)
}

fn sync_folder_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SYNC_FOLDER_FILE_NAME)
}

fn sync_folder_title() -> String {
    std::env::var("BILI_SYNC_FOLDER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_SYNC_FOLDER_TITLE.to_string())
}

fn load_sync_folder_id(data_dir: &Path) -> Option<i64> {
    std::fs::read_to_string(sync_folder_path(data_dir))
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
}

fn save_sync_folder_id(data_dir: &Path, id: i64) -> Result<()> {
    std::fs::create_dir_all(data_dir).context("创建配置目录失败")?;
    std::fs::write(sync_folder_path(data_dir), id.to_string()).context("写入云同步收藏夹 id 失败")
}

fn load_saved_cookie(data_dir: &Path) -> Option<String> {
    std::fs::read_to_string(cookie_path(data_dir))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn save_cookie(data_dir: &Path, cookie: &str) -> Result<()> {
    let path = cookie_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("创建配置目录失败")?;
    }
    std::fs::write(&path, cookie).context("写入 Cookie 文件失败")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn cookie_field(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if key == name && !value.is_empty() {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn cookie_from_set_cookies(headers: &[String]) -> Option<String> {
    let mut parts = Vec::new();
    for header in headers {
        let first = header.split(';').next()?.trim();
        let Some((name, value)) = first.split_once('=') else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        if matches!(
            name,
            "SESSDATA"
                | "bili_jct"
                | "DedeUserID"
                | "DedeUserID__ckMd5"
                | "sid"
                | "buvid3"
                | "buvid4"
        ) {
            parts.push(first.to_string());
        }
    }

    if parts.iter().any(|part| part.starts_with("SESSDATA=")) {
        Some(parts.join("; "))
    } else {
        None
    }
}

fn cookie_from_url(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    let mut parts = Vec::new();
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        if matches!(
            name,
            "SESSDATA" | "bili_jct" | "DedeUserID" | "DedeUserID__ckMd5"
        ) {
            parts.push(pair.to_string());
        }
    }

    if parts.iter().any(|part| part.starts_with("SESSDATA=")) {
        Some(parts.join("; "))
    } else {
        None
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    code: i64,
    message: Option<String>,
    data: Option<SearchData>,
}

#[derive(Deserialize)]
struct SearchData {
    result: Option<Vec<SearchRaw>>,
}

#[derive(Deserialize)]
struct SearchRaw {
    bvid: Option<String>,
    title: Option<String>,
    author: Option<String>,
    duration: Option<String>,
    pic: Option<String>,
}

#[derive(Deserialize)]
struct ViewResponse {
    code: i64,
    message: Option<String>,
    data: Option<ViewData>,
}

#[derive(Deserialize)]
struct ViewData {
    bvid: String,
    #[serde(default)]
    aid: i64,
    cid: i64,
    title: String,
    duration: i64,
    owner: Owner,
    pic: Option<String>,
    #[serde(default)]
    pages: Vec<ViewPage>,
}

#[derive(Deserialize)]
struct ViewPage {
    cid: i64,
    #[serde(default)]
    duration: i64,
    #[serde(default)]
    part: String,
}

#[derive(Deserialize)]
struct Owner {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct PlayUrlResponse {
    code: i64,
    message: Option<String>,
    data: Option<PlayUrlData>,
}

#[derive(Deserialize)]
struct PlayUrlData {
    dash: Option<Dash>,
}

#[derive(Deserialize)]
struct Dash {
    audio: Option<Vec<AudioTrack>>,
    dolby: Option<Dolby>,
    flac: Option<Flac>,
}

#[derive(Deserialize)]
struct Dolby {
    audio: Option<Vec<AudioTrack>>,
}

#[derive(Deserialize)]
struct Flac {
    audio: Option<AudioTrack>,
}

#[derive(Deserialize)]
struct AudioTrack {
    id: i64,
    #[serde(default)]
    bandwidth: i64,
    #[serde(default, rename = "baseUrl")]
    base_url: Option<String>,
    #[serde(default, rename = "backupUrl")]
    backup_url: Vec<String>,
}

#[derive(Deserialize)]
struct NavResponse {
    #[serde(default)]
    code: i64,
    data: Option<NavData>,
}

#[derive(Deserialize)]
struct NavData {
    #[serde(default)]
    is_login: bool,
    uname: Option<String>,
    wbi_img: Option<WbiImg>,
}

#[derive(Deserialize)]
struct WbiImg {
    img_url: String,
    sub_url: String,
}

#[derive(Deserialize)]
struct SpiResponse {
    data: SpiData,
}

#[derive(Deserialize)]
struct SpiData {
    b_3: String,
    b_4: Option<String>,
}

#[derive(Deserialize)]
struct QrGenerateResponse {
    #[serde(default)]
    code: i64,
    message: Option<String>,
    data: Option<QrGenerateData>,
}

#[derive(Deserialize)]
struct QrGenerateData {
    url: String,
    qrcode_key: String,
}

#[derive(Deserialize)]
struct QrPollResponse {
    #[serde(default)]
    code: i64,
    message: Option<String>,
    data: Option<QrPollData>,
}

#[derive(Deserialize)]
struct QrPollData {
    #[serde(default)]
    code: i64,
    message: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct FavoriteFoldersResponse {
    #[serde(default)]
    code: i64,
    message: Option<String>,
    data: Option<FavoriteFoldersData>,
}

#[derive(Deserialize)]
struct FavoriteFoldersData {
    list: Option<Vec<FavoriteFolderRaw>>,
}

#[derive(Deserialize)]
struct FavoriteFolderRaw {
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    media_count: i64,
}

#[derive(Deserialize)]
struct FavoriteItemsResponse {
    #[serde(default)]
    code: i64,
    message: Option<String>,
    data: Option<FavoriteItemsData>,
}

#[derive(Deserialize)]
struct FavoriteItemsData {
    medias: Option<Vec<FavoriteMediaRaw>>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct FavoriteMediaRaw {
    bvid: Option<String>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    duration: i64,
    cover: Option<String>,
    upper: Option<Owner>,
    #[serde(default, rename = "type")]
    media_type: i64,
}

#[derive(Deserialize)]
struct PlayerV2Response {
    #[serde(default)]
    code: i64,
    message: Option<String>,
    data: Option<PlayerV2Data>,
}

#[derive(Deserialize)]
struct PlayerV2Data {
    subtitle: Option<SubtitleData>,
}

#[derive(Deserialize)]
struct SubtitleData {
    subtitles: Option<Vec<SubtitleRaw>>,
}

#[derive(Deserialize)]
struct SubtitleRaw {
    #[serde(default)]
    lan: String,
    subtitle_url: Option<String>,
}

#[derive(Deserialize)]
struct RawLyric {
    #[serde(default, alias = "start")]
    from: f64,
    #[serde(default, alias = "end")]
    to: f64,
    #[serde(default, alias = "text", alias = "body")]
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wbi_mixin_key_matches_reference_vector() {
        let img_key = "7cd084941338484aae1ad9425b84077c";
        let sub_key = "4932caff0ff746eab6f01bf08b70ac45";
        assert_eq!(
            mixin_key(img_key, sub_key),
            "ea1db124af3c7062474693fa704f4ff8"
        );
    }

    #[test]
    fn encoding_encodes_wbi_special_characters() {
        assert_eq!(encode_component("a b!'()*~"), "a%20b%21%27%28%29%2A~");
    }

    #[test]
    fn title_tags_and_entities_are_stripped() {
        assert_eq!(
            clean_title("<em class=\"keyword\">周杰伦</em> &amp; 朋友"),
            "周杰伦 & 朋友"
        );
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_secs(213), "3:33");
        assert_eq!(format_secs(3671), "1:01:11");
        assert_eq!(normalize_duration("419:2"), "6:59:02");
        assert_eq!(normalize_duration("3:33"), "3:33");
    }

    #[test]
    fn lyrics_json_is_parsed_and_blank_lines_dropped() {
        let body = r#"[
            {"from": 0.0, "to": 2.5, "content": "  "},
            {"from": 2.5, "to": 5.0, "content": "第一句"},
            {"from": 5.0, "to": 8.0, "content": "第二句 "}
        ]"#;
        let lines = parse_lyrics(body).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "第一句");
        assert_eq!(lines[1].from, 5.0);
    }

    #[test]
    fn ai_subtitle_object_body_is_parsed() {
        let body = r#"{
            "type": "AIsubtitle",
            "lang": "zh",
            "body": [
                {"from": 12.56, "to": 17.62, "sid": 1, "content": "啊啊啊"}
            ]
        }"#;
        let lines = parse_lyrics(body).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].from, 12.56);
        assert_eq!(lines[0].text, "啊啊啊");
    }

    #[test]
    fn cookie_is_extracted_from_login_url() {
        let url = "https://www.bilibili.com/?SESSDATA=abc%2C123&bili_jct=xyz&DedeUserID=42&gourl=x";
        assert_eq!(
            cookie_from_url(url).unwrap(),
            "SESSDATA=abc%2C123; bili_jct=xyz; DedeUserID=42"
        );
    }

    #[test]
    fn cookie_is_extracted_from_set_cookie_headers() {
        let headers = vec![
            "SESSDATA=abc%2C123; Path=/; Domain=.bilibili.com; HttpOnly".to_string(),
            "bili_jct=xyz; Path=/".to_string(),
            "DedeUserID=42; Path=/".to_string(),
        ];
        assert_eq!(
            cookie_from_set_cookies(&headers).unwrap(),
            "SESSDATA=abc%2C123; bili_jct=xyz; DedeUserID=42"
        );
    }

    #[test]
    fn cookie_fields_are_extracted() {
        let cookie = "SESSDATA=abc%2C123; bili_jct=xyz; DedeUserID=42";
        assert_eq!(cookie_field(cookie, "bili_jct"), Some("xyz".to_string()));
        assert_eq!(cookie_field(cookie, "DedeUserID"), Some("42".to_string()));
        assert_eq!(cookie_field(cookie, "missing"), None);
    }

    #[test]
    fn urls_are_normalized() {
        assert_eq!(
            normalize_url("//i0.hdslb.com/a.jpg"),
            "https://i0.hdslb.com/a.jpg"
        );
        assert_eq!(
            normalize_url("http://i0.hdslb.com/a.jpg"),
            "https://i0.hdslb.com/a.jpg"
        );
        assert_eq!(
            normalize_url("https://i0.hdslb.com/a.jpg"),
            "https://i0.hdslb.com/a.jpg"
        );
        assert_eq!(
            cover_thumbnail_url("http://i1.hdslb.com/a.jpg"),
            "https://i1.hdslb.com/a.jpg@640w_360h.jpg"
        );
        assert_eq!(
            cover_thumbnail_url("https://i0.hdslb.com/a.jpg@320w.jpg"),
            "https://i0.hdslb.com/a.jpg@320w.jpg"
        );
        assert_eq!(
            cover_thumbnail_url("https://i0.hdslb.com/a.jpg?x=1"),
            "https://i0.hdslb.com/a.jpg@640w_360h.jpg?x=1"
        );
    }
}
