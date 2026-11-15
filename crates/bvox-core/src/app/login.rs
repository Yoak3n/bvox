//! Login flow state: QR modal and account info.

use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};

use super::App;
use super::LoginModal;
use super::MainView;
use crate::api::UserInfo;

impl App {
    pub fn open_login(&mut self) -> u64 {
        self.login_seq += 1;
        self.login = Some(LoginModal {
            qr: Vec::new(),
            status: "正在生成二维码…".to_string(),
        });
        self.set_status("正在生成登录二维码…", false);
        self.login_seq
    }

    pub fn on_login_qr(&mut self, seq: u64, matrix: Vec<Vec<bool>>) {
        if seq != self.login_seq {
            return;
        }
        if let Some(modal) = &mut self.login {
            modal.qr = matrix;
            modal.status = "用 B 站手机客户端扫码".to_string();
        }
        self.set_status("等待扫码…", false);
    }

    pub fn on_login_phase(&mut self, seq: u64, status: String) {
        if seq != self.login_seq {
            return;
        }
        if let Some(modal) = &mut self.login {
            modal.status = status;
        }
    }

    pub fn on_login_success(&mut self, seq: u64, user: Option<UserInfo>) -> bool {
        if seq != self.login_seq {
            return false;
        }
        self.login = None;
        let name = user
            .as_ref()
            .map(|user| user.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "已登录".to_string());
        self.vip = user.as_ref().is_some_and(|user| user.vip);
        self.user_face = user.as_ref().and_then(|user| user.face.clone());
        self.user = Some(name.clone());
        self.view = MainView::Library;
        self.set_status(&format!("已登录：{name}"), false);
        true
    }

    pub fn on_login_failed(&mut self, seq: u64, message: String) {
        if seq != self.login_seq {
            return;
        }
        self.login = None;
        self.set_status(&format!("登录失败: {message}"), true);
    }
}
pub fn qr_matrix(url: &str) -> Option<Vec<Vec<bool>>> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::L).ok()?;
    let width = code.width();
    let modules = code.to_colors();
    let quiet = 4;
    let size = width + quiet * 2;
    let mut matrix = vec![vec![false; size]; size];

    for y in 0..width {
        for x in 0..width {
            matrix[y + quiet][x + quiet] = modules[y * width + x] == QrColor::Dark;
        }
    }

    Some(matrix)
}
