//! Cookie persistence and QR-code login.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::sign::encode_component;
use super::text::snippet;
use super::{ApiClient, QrLogin, QrPhase, REFERER, UA};

const COOKIE_FILE_NAME: &str = "cookie";
impl ApiClient {
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

    /// Forget the stored login (in memory and on disk).
    pub async fn clear_cookie(&self) {
        *self.cookie.lock().await = None;
        let _ = std::fs::remove_file(cookie_path(&self.paths.data_dir));
    }
}
impl ApiClient {
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
}
pub(super) fn cookie_path(data_dir: &Path) -> PathBuf {
    data_dir.join(COOKIE_FILE_NAME)
}
pub(super) fn load_saved_cookie(data_dir: &Path) -> Option<String> {
    std::fs::read_to_string(cookie_path(data_dir))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn save_cookie(data_dir: &Path, cookie: &str) -> Result<()> {
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
pub(super) fn cookie_from_set_cookies(headers: &[String]) -> Option<String> {
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

pub(super) fn cookie_from_url(url: &str) -> Option<String> {
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
pub(super) struct QrGenerateResponse {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<QrGenerateData>,
}

#[derive(Deserialize)]
pub(super) struct QrGenerateData {
    pub(super) url: String,
    pub(super) qrcode_key: String,
}

#[derive(Deserialize)]
pub(super) struct QrPollResponse {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<QrPollData>,
}

#[derive(Deserialize)]
pub(super) struct QrPollData {
    #[serde(default)]
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) url: Option<String>,
}
