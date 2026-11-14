//! Account commands: login, cookie handling and logout.

use crate::session::Session;
use crate::session::tasks::login::spawn_login;

impl Session {
    pub fn login(&self, seq: u64) {
        spawn_login(&self.api, self.tx.clone(), seq);
    }

    // ----- cache management ------------------------------------------------
}
impl Session {
    /// Open the QR login modal and start polling (TUI's `g`).
    pub fn open_login(&mut self) {
        let seq = self.app.open_login();
        spawn_login(&self.api, self.tx.clone(), seq);
    }

    /// Close the login modal and invalidate the polling task.
    pub fn cancel_login(&mut self) {
        if self.app.login.is_some() {
            self.app.login_seq += 1;
            self.app.login = None;
            self.app.set_status("已取消登录", false);
        }
    }

    /// Install a cookie obtained outside the QR flow (in-app WebView or
    /// pasted by the user), then refresh the account state.
    pub async fn set_cookie_text(&mut self, cookie: String) {
        self.api.set_cookie(cookie).await;
        self.app.has_cookie = true;
        self.app.cancel_pending_import();
        match self.api.user_info().await {
            Ok(Some(user)) => {
                self.app.vip = user.vip;
                self.app.user_face = user.face;
                self.app.user = Some(user.name.clone());
                self.app
                    .set_status(&format!("已登录：{}", user.name), false);
            }
            Ok(None) => {
                self.app.vip = false;
                self.app
                    .set_status("Cookie 已保存，但未能确认登录状态", true);
            }
            Err(error) => {
                self.app
                    .set_status(&format!("Cookie 已保存，用户信息获取失败: {error:#}"), true);
            }
        }
        // Switching accounts must not keep the previous account's folders
        // visible; swap in the new account's cache (if any) until the refresh
        // lands.
        self.app.playlists.clear();
        if let Some(folders) = self.api.cached_favorite_folders().await {
            let hidden = self.api.hidden_playlists().await;
            self.app.load_cached_playlists(folders, hidden);
        }
        self.app.open_library();
        self.refresh_playlists();
    }

    /// Sign out: forget the cookie and the cached account.
    pub async fn logout(&mut self) {
        self.api.clear_cookie().await;
        self.app.cancel_pending_import();
        self.app.invalidate_playlist_requests();
        self.app.has_cookie = false;
        self.app.user = None;
        self.app.user_face = None;
        self.app.vip = false;
        self.app.playlists.clear();
        self.app.hidden_playlists.clear();
        self.app.show_hidden = false;
        self.app.playlist_selected = None;
        self.app.current_playlist = None;
        self.app.open_queue();
        self.app.set_status("已退出登录", false);
        self.save_playback();
    }
}
