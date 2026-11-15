package dev.yoa.bvox.ui.login

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.LoginModal
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.ui.login.QrCodeView

@Composable
internal fun QrLoginTab(modal: LoginModal?) {
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
