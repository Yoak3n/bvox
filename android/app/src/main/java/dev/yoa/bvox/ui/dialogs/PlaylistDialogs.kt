package dev.yoa.bvox.ui.dialogs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.FavoriteFolder

@Composable
internal fun PlaylistNameDialog(
    title: String,
    initial: String,
    confirmLabel: String,
    hint: String? = null,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    var input by remember(initial) { mutableStateOf(initial) }
    val trimmed = input.trim()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                if (hint != null) {
                    Text(
                        hint,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(bottom = 8.dp),
                    )
                }
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    singleLine = true,
                    label = { Text("歌单名") },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = trimmed.isNotEmpty(),
                onClick = { onConfirm(trimmed) },
            ) { Text(confirmLabel) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("取消") }
        },
    )
}

/** Pick a favorite folder to add the selected track to. */
@Composable
internal fun AddToPlaylistDialog(
    trackTitle: String,
    folders: List<FavoriteFolder>,
    onDismiss: () -> Unit,
    onCreateNew: (() -> Unit)? = null,
    onPick: (FavoriteFolder) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("加入歌单") },
        text = {
            Column {
                Text(
                    trackTitle,
                    style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                if (folders.isEmpty()) {
                    Text(
                        if (onCreateNew != null) {
                            "还没有歌单，可以先新建一个"
                        } else {
                            "还没有歌单，请先在歌单列表新建"
                        },
                        modifier = Modifier.padding(16.dp),
                    )
                } else {
                    LazyColumn(modifier = Modifier.fillMaxWidth().heightIn(max = 320.dp)) {
                        items(folders, key = { it.id }) { folder ->
                            ListItem(
                                headlineContent = { Text(folder.title) },
                                supportingContent = { Text("${folder.count} 首") },
                                modifier = Modifier.clickable { onPick(folder) },
                            )
                        }
                    }
                }
                if (onCreateNew != null) {
                    ListItem(
                        headlineContent = { Text("＋ 新建歌单") },
                        modifier = Modifier.clickable { onCreateNew() },
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("取消") }
        },
    )
}

/** Pick one 分P of the currently playing multi-page video. */
