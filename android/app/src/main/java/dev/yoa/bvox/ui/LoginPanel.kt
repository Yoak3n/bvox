package dev.yoa.bvox.ui

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
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
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

import dev.yoa.bvox.core.LoginModal
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.ui.login.QrCodeView



/**
 * Login panel. QR comes first (two-device scan is the smoothest path), then
 * the in-app WebView login (phone/SMS, for single-device use), then a manual
 * cookie paste fallback.
 */
@Composable
fun LoginPanel(modal: LoginModal?, onDismiss: () -> Unit) {
    var tab by remember { mutableIntStateOf(0) }
    val tabs = listOf("扫码登录", "网页登录", "Cookie")

    Dialog(
        onDismissRequest = {
            // No-op unless a QR poll loop is running.
            CoreHolder.core.cancelLogin()
            onDismiss()
        },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        // The web login page positions its agreement row relative to the
        // viewport height; a short WebView (480dp) collapses that layout onto
        // the login button. Give that tab a near-fullscreen card so the
        // viewport resembles a real phone (Chrome: ~890dp tall).
        val wide = tab == 1
        Surface(
            shape = RoundedCornerShape(20.dp),
            tonalElevation = 6.dp,
            modifier = if (wide) {
                Modifier
                    .fillMaxWidth(0.94f)
                    .widthIn(max = 520.dp)
                    .fillMaxHeight(0.82f)
            } else {
                Modifier
                    .fillMaxWidth(0.92f)
                    .widthIn(max = 460.dp)
                    .padding(vertical = 24.dp)
            },
        ) {
            Column(
                modifier = Modifier
                    .padding(
                        horizontal = if (wide) 10.dp else 16.dp,
                        vertical = 16.dp,
                    )
                    .then(if (wide) Modifier.fillMaxHeight() else Modifier),
            ) {
                Text("登录 B 站", style = MaterialTheme.typography.titleLarge)
                Spacer(Modifier.height(12.dp))

                TabRow(selectedTabIndex = tab) {
                    tabs.forEachIndexed { index, title ->
                        Tab(
                            selected = tab == index,
                            onClick = {
                                if (index != tab) {
                                    // Leaving the QR tab stops its poll loop;
                                    // entering it starts a fresh one.
                                    if (tab == 0) CoreHolder.core.cancelLogin()
                                    if (index == 0) CoreHolder.core.startLogin()
                                    tab = index
                                }
                            },
                            text = { Text(title) },
                        )
                    }
                }
                Spacer(Modifier.height(12.dp))

                when (tab) {
                    0 -> QrLoginTab(modal)
                    1 -> WebViewLogin(
                        onCookie = { cookie ->
                            CoreHolder.core.setCookie(cookie)
                            onDismiss()
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f),
                    )
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
    // logging in. The rest of the cookie set (bili_jct, DedeUserID, ...) is
    // written a beat later on the redirect, so wait briefly and gather the
    // jar from every bilibili domain before handing it to the core.
    LaunchedEffect(Unit) {
        while (isActive) {
            delay(1000)
            val cookie = CookieManager.getInstance()
                .getCookie("https://www.bilibili.com")
            if (cookie != null && cookie.contains("SESSDATA=")) {
                val full = readFullCookie()
                if (full != null) {
                    onCookie(full)
                    break
                }
            }
        }
    }

    AndroidView(
        modifier = modifier,
        factory = { context ->
            // Mobile-first login page; the PC page is the fallback if the
            // h5 variant fails to load.
            val h5Login = "https://passport.bilibili.com/h5-app/passport/login"
            val pcLogin = "https://passport.bilibili.com/login"
            var fellBack = false

            WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                // Respect the page's <meta viewport> and render like mobile
                // Chrome: bilibili serves a cleaner browser variant when the
                // UA lacks the WebView markers ("wv", "Version/4.0"). Without
                // this the embedded-app variant overlaps the agreement row
                // with the login button.
                settings.useWideViewPort = true
                settings.loadWithOverviewMode = true
                settings.userAgentString =
                    "Mozilla/5.0 (Linux; Android 16; 23127PN0CC) " +
                    "AppleWebKit/537.36 (KHTML, like Gecko) " +
                    "Chrome/155.0.8059.43 Mobile Safari/537.36"
                // WebView inherits the system font scale; forcing 100% keeps
                // the page's fixed-height rows intact.
                settings.textZoom = 100
                CookieManager.getInstance().setAcceptCookie(true)
                CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
                webViewClient = object : WebViewClient() {
                    override fun onPageFinished(view: WebView, url: String) {
                        // The page absolutely-positions the agreement row
                        // (.explain-tips) with JS-computed top/bottom offsets
                        // that assume a browser viewport; in this WebView the
                        // offsets collapse onto the login button. Put the row
                        // back into normal flow so it sits below the form,
                        // and keep the scroll root full-height.
                        view.postDelayed({
                            view.evaluateJavascript(
                                "(function(){" +
                                    "var st=document.createElement('style');" +
                                    "st.textContent=" +
                                    "'html,body{height:100%!important;min-height:100%!important}'+" +
                                    "'.explain-tips{position:static!important;height:auto!important;" +
                                    "margin-top:16px!important;bottom:auto!important;top:auto!important}'+" +
                                    "'.form-btn.login-btn{margin-top:24px!important}';" +
                                    "document.documentElement.appendChild(st);" +
                                    "})()",
                                null,
                            )
                        }, 1200)
                    }

                    override fun shouldOverrideUrlLoading(
                        view: WebView,
                        request: WebResourceRequest,
                    ): Boolean {
                        val scheme = request.url.scheme ?: return true
                        if (scheme == "http" || scheme == "https") {
                            return false
                        }
                        // Keep the page inside the WebView instead of letting
                        // it hand off to the bilibili:// app (blank page).
                        Log.i("Bvox", "blocked scheme: ${request.url}")
                        return true
                    }

                    override fun onReceivedError(
                        view: WebView,
                        request: WebResourceRequest,
                        error: WebResourceError,
                    ) {
                        Log.w(
                            "Bvox",
                            "WebView 加载失败 ${request.url}: ${error.description}",
                        )
                        if (request.isForMainFrame && !fellBack) {
                            fellBack = true
                            view.loadUrl(pcLogin)
                        }
                    }
                }
                loadUrl(h5Login)
            }
        },
        onRelease = { webView ->
            webView.stopLoading()
            webView.destroy()
        },
    )
}

/**
 * Merge cookies from every bilibili domain. The login redirect sets the full
 * cookie set (bili_jct, DedeUserID, ...) a beat after SESSDATA appears, so a
 * short wait plus a multi-domain gather avoids saving a partial cookie.
 */
private suspend fun readFullCookie(): String? {
    delay(1200)
    val manager = CookieManager.getInstance()
    val parts = linkedMapOf<String, String>()
    for (url in listOf(
        "https://www.bilibili.com",
        "https://passport.bilibili.com",
        "https://api.bilibili.com",
    )) {
        val cookie = manager.getCookie(url) ?: continue
        for (pair in cookie.split(";")) {
            val trimmed = pair.trim()
            val index = trimmed.indexOf('=')
            if (index > 0) {
                parts[trimmed.substring(0, index)] = trimmed
            }
        }
    }
    return if (parts.isEmpty()) null else parts.values.joinToString("; ")
}

@Composable
private fun QrLoginTab(modal: LoginModal?) {
    // The QR tab is the default one; start polling on first composition.
    LaunchedEffect(Unit) {
        if (modal == null) {
            CoreHolder.core.startLogin()
        }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (modal == null || modal.qr.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(260.dp),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        } else {
            QrCodeView(
                matrix = modal.qr,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(260.dp),
            )
        }
        Text(
            modal?.status ?: "正在生成二维码…",
            modifier = Modifier.padding(top = 8.dp),
            style = MaterialTheme.typography.bodySmall,
        )
        Text(
            "用 B 站手机客户端扫码；同一台手机请用「网页登录」",
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
    Column(modifier = Modifier.padding(vertical = 4.dp)) {
        Text(
            "在浏览器登录 bilibili.com 后，复制整段 Cookie 粘贴到这里（需包含 SESSDATA 和 bili_jct）",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedTextField(
            value = input,
            onValueChange = { input = it },
            modifier = Modifier
                .fillMaxWidth()
                .height(160.dp)
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
