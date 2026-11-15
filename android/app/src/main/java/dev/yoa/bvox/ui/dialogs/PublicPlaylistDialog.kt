package dev.yoa.bvox.ui.dialogs

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.CoreHolder

/** Import another user's public favorite folder as a read-only queue. */
@Composable
internal fun PublicPlaylistDialog(onDismiss: () -> Unit) {
    var input by remember { mutableStateOf("") }
    val trimmed = input.trim()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("导入公开收藏夹") },
        text = {
            Column {
                Text(
                    text = "粘贴别人的公开收藏夹链接或 media_id。导入后作为只读播放队列，" +
                        "不会改动对方收藏夹。",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(bottom = 12.dp),
                )
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    singleLine = true,
                    label = { Text("收藏夹链接 / media_id") },
                    placeholder = { Text("space.bilibili.com/…/favlist?fid=…") },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = trimmed.isNotEmpty(),
                onClick = {
                    CoreHolder.core.importPublicPlaylist(trimmed)
                    onDismiss()
                },
            ) { Text("导入") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("取消") }
        },
    )
}
