package dev.yoa.bilimusic.ui

import android.util.Log
import android.webkit.CookieManager
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.yoa.bilimusic.CoreHolder
import dev.yoa.bilimusic.core.LoginModal
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

/**
 * Login panel with three paths, in order of convenience on a single device:
 *  1. 网页登录 — an in-app WebView running bilibili.com's passport page
 *     (phone/SMS login). The cookie is picked up automatically.
 *  2. 扫码 — the original QR flow, useful when a second device scans it.
 *  3. Cookie — manual fallback copied from a browser.
 */
@Composable
fun LoginPanel(modal: LoginModal?, onDismiss: () -> Unit) {
    var tab by remember { mutableIntStateOf(0) }
    val tabs = listOf("网页登录", "扫码", "Cookie")

    Dialog(
        onDismissRequest = {
            // No-op unless a QR poll loop is running.
            CoreHolder.core.cancelLogin()
            onDismiss()
        },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Surface(
            shape = RoundedCornerShape(20.dp),
            tonalElevation = 6.dp,
            modifier = Modifier
                .fillMaxWidth(0.94f)
                .padding(vertical = 24.dp),
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text("登录 B 站", style = MaterialTheme.typography.titleLarge)
                Spacer(Modifier.height(12.dp))

                TabRow(selectedTabIndex = tab) {
                    tabs.forEachIndexed { index, title ->
                        Tab(
                            selected = tab == index,
                            onClick = {
                                tab = index
                                if (index == 1 && modal == null) {
                                    CoreHolder.core.startLogin()
                                }
                            },
                            text = { Text(title) },
                        )
                    }
                }
                Spacer(Modifier.height(12.dp))

                when (tab) {
                    0 -> WebViewLogin(
                        onCookie = { cookie ->
                            CoreHolder.core.setCookie(cookie)
                            onDismiss()
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(440.dp),
                    )
                    1 -> QrLoginTab(modal)
                    2 -> CookiePasteTab(onSubmit = { cookie ->
                        CoreHolder.core.setCookie(cookie)
                        onDismiss()
                    })
                }

                Spacer(Modifier.height(4.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
                    TextButton(onClick = {
                        CoreHolder.core.cancelLogin()
                        onDismiss()
                    }) { Text("关闭") }
                }
            }
        }
    }
}

@Composable
private fun WebViewLogin(onCookie: (String) -> Unit, modifier: Modifier = Modifier) {
    // Poll the WebView cookie jar; SESSDATA appears once the user finishes
    // logging in on the passport page.
    LaunchedEffect(Unit) {
        while (isActive) {
            delay(1000)
            val cookie = CookieManager.getInstance()
                .getCookie("https://www.bilibili.com")
            if (cookie != null && cookie.contains("SESSDATA=")) {
                onCookie(cookie)
                break
            }
        }
    }

    AndroidView(
        modifier = modifier,
        factory = { context ->
            WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                CookieManager.getInstance().setAcceptCookie(true)
                CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
                webViewClient = object : WebViewClient() {
                    override fun onReceivedError(
                        view: WebView,
                        request: WebResourceRequest,
                        error: WebResourceError,
                    ) {
                        Log.w(
                            "BiliMusic",
                            "WebView 加载失败 ${request.url}: ${error.description}",
                        )
                    }
                }
                loadUrl("https://passport.bilibili.com/login")
            }
        },
        onRelease = { webView ->
            webView.stopLoading()
            webView.destroy()
        },
    )
}

@Composable
private fun QrLoginTab(modal: LoginModal?) {
    // Starting the poll loop is triggered by selecting the tab; guard against
    // double-starting when the modal is already present.
    LaunchedEffect(Unit) {
        if (modal == null) {
            CoreHolder.core.startLogin()
        }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (modal == null || modal.qr.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(240.dp),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        } else {
            QrCodeView(
                matrix = modal.qr,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(240.dp),
            )
        }
        Text(
            modal?.status ?: "正在生成二维码…",
            modifier = Modifier.padding(vertical = 6.dp),
            style = MaterialTheme.typography.bodySmall,
        )
        Text(
            "二维码适合用另一台设备扫描；同一台手机请用「网页登录」",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        TextButton(onClick = { CoreHolder.core.startLogin() }) {
            Text("刷新二维码")
        }
    }
}

@Composable
private fun CookiePasteTab(onSubmit: (String) -> Unit) {
    var input by remember { mutableStateOf("") }
    Column(modifier = Modifier.padding(vertical = 8.dp)) {
        Text(
            "在浏览器登录 bilibili.com 后，复制整段 Cookie 粘贴到这里（需包含 SESSDATA）",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedTextField(
            value = input,
            onValueChange = { input = it },
            modifier = Modifier
                .fillMaxWidth()
                .height(180.dp)
                .padding(vertical = 8.dp),
            placeholder = { Text("SESSDATA=xxx; bili_jct=yyy; DedeUserID=zzz; ...") },
        )
        Button(
            onClick = {
                val text = input.trim()
                if (text.isNotEmpty()) onSubmit(text)
            },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("保存 Cookie")
        }
    }
}
