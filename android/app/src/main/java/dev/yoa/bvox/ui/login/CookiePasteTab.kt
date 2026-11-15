package dev.yoa.bvox.ui.login

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

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
