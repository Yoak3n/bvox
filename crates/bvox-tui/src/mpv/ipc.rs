//! mpv JSON IPC plumbing: socket handshake and command round-trips.

use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::time::timeout;

use super::Mpv;
use super::harden_command;

impl Mpv {
    pub(super) async fn number(&mut self, property: &str) -> Option<f64> {
        self.get(property).await.and_then(|value| value.as_f64())
    }

    pub(super) async fn get(&mut self, property: &str) -> Option<Value> {
        self.command(json!({"command": ["get_property", property]}))
            .await
            .ok()
            .flatten()
    }

    pub(super) async fn ensure_started(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        let _ = std::fs::remove_file(&self.socket);

        let mut command = Command::new("mpv");
        command
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--force-window=no")
            .arg("--no-terminal")
            .arg("--really-quiet")
            .arg("--keep-open=no")
            .arg(format!("--referrer={}", self.config.referer))
            .arg(format!("--user-agent={}", self.config.user_agent))
            .arg("--cache=yes")
            .arg("--cache-pause=yes")
            .arg(format!("--input-ipc-server={}", self.socket.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        harden_command(&mut command);
        let child = command.spawn().context("无法启动 mpv，请先安装 mpv")?;
        self.child = Some(child);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            match UnixStream::connect(&self.socket).await {
                Ok(stream) => {
                    let (read_half, write_half) = stream.into_split();
                    self.reader = Some(BufReader::new(read_half));
                    self.writer = Some(write_half);
                    return Ok(());
                }
                Err(error) => {
                    if tokio::time::Instant::now() >= deadline {
                        bail!("连接 mpv IPC 失败: {error}");
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }

    pub(super) async fn command(&mut self, mut command: Value) -> Result<Option<Value>> {
        self.request_id += 1;
        let request_id = self.request_id;
        command["request_id"] = json!(request_id);
        let payload = format!("{}\n", command);

        let (Some(reader), Some(writer)) = (self.reader.as_mut(), self.writer.as_mut()) else {
            bail!("mpv 未启动");
        };
        writer
            .write_all(payload.as_bytes())
            .await
            .context("写入 mpv IPC 失败")?;
        writer.flush().await?;

        let read_response = async {
            let mut line = String::new();
            loop {
                line.clear();
                let read = reader.read_line(&mut line).await?;
                if read == 0 {
                    bail!("mpv IPC 已关闭");
                }
                let Ok(message) = serde_json::from_str::<Value>(line.trim()) else {
                    continue;
                };
                if message.get("request_id").and_then(Value::as_u64) != Some(request_id) {
                    continue;
                }
                if message.get("error").and_then(Value::as_str) == Some("success") {
                    return Ok(message.get("data").cloned());
                }
                bail!(
                    "mpv: {}",
                    message
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("未知错误")
                );
            }
        };

        match timeout(Duration::from_secs(3), read_response).await {
            Ok(result) => result,
            Err(_) => bail!("mpv IPC 超时"),
        }
    }

    pub(super) fn reset(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        self.child = None;
        self.reader = None;
        self.writer = None;
        let _ = std::fs::remove_file(&self.socket);
    }
}
