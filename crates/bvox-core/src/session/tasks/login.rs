//! Background login and account-info tasks.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::api::{ApiClient, QrPhase};
use crate::app::qr_matrix;
use crate::session::msg::Msg;

pub(in crate::session) fn spawn_user_info(api: &Arc<ApiClient>, tx: mpsc::UnboundedSender<Msg>) {
    let api = api.clone();
    tokio::spawn(async move {
        // The first request right after process start can lose to a slow
        // network; retry a couple of times before giving up on the name.
        let mut user = None;
        for attempt in 0..3 {
            match api.user_info().await {
                Ok(info) => {
                    user = info;
                    break;
                }
                Err(_) if attempt < 2 => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(Msg::UserInfo(user));
    });
}
pub(in crate::session) fn spawn_login(
    api: &Arc<ApiClient>,
    tx: mpsc::UnboundedSender<Msg>,
    seq: u64,
) {
    let api = api.clone();
    tokio::spawn(async move {
        let qr = match api.qr_generate().await {
            Ok(qr) => qr,
            Err(error) => {
                let _ = tx.send(Msg::LoginFailed {
                    seq,
                    message: format!("{error:#}"),
                });
                return;
            }
        };

        let Some(matrix) = qr_matrix(&qr.url) else {
            let _ = tx.send(Msg::LoginFailed {
                seq,
                message: "二维码生成失败".to_string(),
            });
            return;
        };
        if tx.send(Msg::LoginQr { seq, matrix }).is_err() {
            return;
        }

        let mut last_phase = String::new();
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match api.qr_poll(&qr.key).await {
                Ok(QrPhase::Success) => {
                    let user = api.user_info().await.ok().flatten();
                    let _ = tx.send(Msg::LoginSuccess { seq, user });
                    return;
                }
                Ok(QrPhase::Expired) => {
                    let _ = tx.send(Msg::LoginPhase {
                        seq,
                        status: "二维码已过期，重新打开登录".to_string(),
                    });
                    return;
                }
                Ok(QrPhase::Scanned) => {
                    if last_phase != "scanned" {
                        last_phase = "scanned".to_string();
                        let _ = tx.send(Msg::LoginPhase {
                            seq,
                            status: "已扫码，请在手机上确认".to_string(),
                        });
                    }
                }
                Ok(QrPhase::Waiting) => {
                    if last_phase != "waiting" {
                        last_phase = "waiting".to_string();
                        let _ = tx.send(Msg::LoginPhase {
                            seq,
                            status: "等待扫码…".to_string(),
                        });
                    }
                }
                Err(error) => {
                    if last_phase != "error" {
                        last_phase = "error".to_string();
                        let _ = tx.send(Msg::LoginPhase {
                            seq,
                            status: format!("状态查询失败，重试中: {error:#}"),
                        });
                    }
                }
            }
        }

        let _ = tx.send(Msg::LoginPhase {
            seq,
            status: "二维码已过期，重新打开登录".to_string(),
        });
    });
}
