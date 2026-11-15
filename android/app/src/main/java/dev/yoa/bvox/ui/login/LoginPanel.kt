package dev.yoa.bvox.ui.login

import android.webkit.WebView
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

import dev.yoa.bvox.core.LoginModal
import dev.yoa.bvox.core.CoreHolder

@Composable
internal fun LoginPanel(modal: LoginModal?, onDismiss: () -> Unit) {
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
