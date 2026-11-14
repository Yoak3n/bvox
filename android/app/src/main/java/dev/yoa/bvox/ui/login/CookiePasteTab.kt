package dev.yoa.bvox.ui.login

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


@Composable
internal fun CookiePasteTab(onSubmit: (String) -> Unit) {
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
