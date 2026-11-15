package dev.yoa.bvox.ui.login

import android.util.Log
import android.webkit.CookieManager
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView

import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

@Composable
internal fun WebViewLogin(onCookie: (String) -> Unit, modifier: Modifier = Modifier) {
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
