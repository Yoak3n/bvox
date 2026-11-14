//! HTTP transport: DNS selection, cookies, WBI keys and response checks.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;

use super::account::NavResponse;
use super::sign::extract_key;
use super::text::snippet;
use super::{ApiClient, REFERER, UA};

/// Prefer IPv4 and put a reachable address first. Some Bilibili CDN nodes
/// publish addresses that accept DNS but silently drop connections, and
/// reqwest does not perform Happy Eyeballs fallback by itself.
#[derive(Debug)]
pub(super) struct Ipv4FirstResolver {
    cache: Arc<Mutex<HashMap<String, (Vec<SocketAddr>, Instant)>>>,
}

impl Ipv4FirstResolver {
    pub(super) fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) async fn forget(&self, host: &str) {
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
pub(super) async fn first_reachable(host: &str, addrs: &[SocketAddr]) -> Option<SocketAddr> {
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
impl ApiClient {
    pub(super) async fn csrf_token(&self) -> Result<String> {
        let cookie = self
            .cookie
            .lock()
            .await
            .clone()
            .context("未登录，请先登录")?;
        cookie_field(&cookie, "bili_jct")
            .filter(|value| !value.is_empty())
            .context("Cookie 缺少 bili_jct，请重新登录")
    }

    pub(super) async fn post_form(&self, url: &str, body: String) -> Result<Value> {
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
}
impl ApiClient {
    pub(super) async fn mid(&self) -> Option<i64> {
        let cookie = self.cookie.lock().await.clone()?;
        cookie_field(&cookie, "DedeUserID").and_then(|value| value.parse::<i64>().ok())
    }

    pub(super) async fn wbi_keys(&self) -> Result<(String, String)> {
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
    pub(super) async fn buvid_extra(&self) -> Result<Option<String>> {
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

    pub(super) async fn get_text(&self, url: &str, extra_cookie: Option<&str>) -> Result<String> {
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

    pub(super) async fn get_json<T>(&self, url: &str, extra_cookie: Option<&str>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let body = self.get_text(url, extra_cookie).await?;
        match serde_json::from_str(&body) {
            Ok(value) => Ok(value),
            Err(error) => {
                // A schema mismatch used to be reported as risk control; keep
                // the distinction so the next API change is diagnosable.
                let hint = if error.is_syntax() || error.is_eof() {
                    "响应不是 JSON（可能被风控）"
                } else {
                    "响应结构与预期不符"
                };
                bail!("{hint}: {error}; 响应片段: {}", snippet(&body))
            }
        }
    }

    pub(super) async fn combine_cookie(&self, extra: Option<&str>) -> Option<String> {
        let stored = self.cookie.lock().await.clone();
        match (stored, extra) {
            (Some(base), Some(extra)) => Some(format!("{base}; {extra}")),
            (Some(base), None) => Some(base),
            (None, Some(extra)) => Some(extra.to_string()),
            (None, None) => None,
        }
    }
}
/// Check the `code` of a JSON API response and turn non-zero into an error
/// carrying Bilibili's message.
pub(super) fn check_response(response: &Value, action: &str) -> Result<()> {
    let code = response.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code == 0 {
        return Ok(());
    }
    bail!(
        "{action} {code}: {}",
        response
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("未知错误")
    )
}
pub(super) fn cookie_field(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if key == name && !value.is_empty() {
            Some(value.to_string())
        } else {
            None
        }
    })
}
#[derive(Deserialize)]
pub(super) struct WbiImg {
    pub(super) img_url: String,
    pub(super) sub_url: String,
}

#[derive(Deserialize)]
pub(super) struct SpiResponse {
    pub(super) data: SpiData,
}

#[derive(Deserialize)]
pub(super) struct SpiData {
    pub(super) b_3: String,
    pub(super) b_4: Option<String>,
}
