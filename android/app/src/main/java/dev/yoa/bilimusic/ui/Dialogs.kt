package dev.yoa.bilimusic.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.yoa.bilimusic.CoreHolder
import dev.yoa.bilimusic.core.FavoritesState
import dev.yoa.bilimusic.core.LoginModal

/** QR login modal, driven by `snapshot.login` (core state). */
@Composable
fun LoginDialog(modal: LoginModal) {
    AlertDialog(
        onDismissRequest = { CoreHolder.core.cancelLogin() },
        title = { Text("扫码登录") },
        text = {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                if (modal.qr.isEmpty()) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(24.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator()
                    }
                } else {
                    QrCodeView(
                        matrix = modal.qr,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 8.dp),
                    )
                }
                Text(
                    modal.status,
                    modifier = Modifier.padding(vertical = 8.dp),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { CoreHolder.core.startLogin() }) {
                Icon(Icons.Filled.Refresh, contentDescription = null)
                Text(" 重新生成")
            }
        },
        dismissButton = {
            TextButton(onClick = { CoreHolder.core.cancelLogin() }) {
                Text("关闭")
            }
        },
    )
}

/** Favorites folder picker, driven by `snapshot.favorites` (core state). */
@Composable
fun FavoritesDialog(modal: FavoritesState) {
    AlertDialog(
        onDismissRequest = { CoreHolder.core.closeFavorites() },
        title = { Text("我的收藏夹") },
        text = {
            Column {
                if (modal.folders.isEmpty()) {
                    Text(
                        if (modal.loading) "加载中…" else "（空）",
                        modifier = Modifier.padding(16.dp),
                    )
                    if (modal.loading) {
                        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    }
                } else {
                    LazyColumn(modifier = Modifier.size(height = 320.dp, width = 400.dp)) {
                        items(modal.folders, key = { it.id }) { folder ->
                            ListItem(
                                headlineContent = { Text(folder.title) },
                                supportingContent = { Text("${folder.count} 个内容") },
                                modifier = Modifier.clickable {
                                    CoreHolder.core.importFolder(folder.id, folder.title)
                                },
                            )
                        }
                    }
                }
                if (modal.status.isNotEmpty()) {
                    Text(
                        modal.status,
                        modifier = Modifier.padding(top = 8.dp),
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { CoreHolder.core.closeFavorites() }) {
                Text("关闭")
            }
        },
    )
}

/** BV 号 / 链接导入 (the TUI's `b` key). */
@Composable
fun BvImportDialog(onDismiss: () -> Unit) {
    var input by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("导入 BV 号/链接") },
        text = {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                singleLine = true,
                label = { Text("BV 号或视频链接") },
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val text = input.trim()
                    if (text.isNotEmpty()) {
                        CoreHolder.core.importBv(text)
                        onDismiss()
                    }
                },
            ) { Text("导入") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("取消") }
        },
    )
}
