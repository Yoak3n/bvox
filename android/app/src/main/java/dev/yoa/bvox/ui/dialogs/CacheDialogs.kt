package dev.yoa.bvox.ui.dialogs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.CacheMode
import dev.yoa.bvox.prefs.CachePrefs

@Composable
internal fun CacheModeDialog(
    current: CacheMode,
    onDismiss: () -> Unit,
    onSelect: (CacheMode) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("缓存策略") },
        text = {
            Column {
                Text(
                    "关闭或切换策略不会删除已有文件，可用「清理」释放空间。",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                CachePrefs.MODE_OPTIONS.forEach { mode ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(mode) }
                            .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(
                            selected = mode == current,
                            onClick = { onSelect(mode) },
                        )
                        Column(modifier = Modifier.padding(start = 8.dp)) {
                            Text(CachePrefs.modeLabel(mode))
                            Text(
                                text = CachePrefs.modeDescription(mode),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("关闭") }
        },
    )
}

/** Disk cache cap; lowering it evicts least recently used files at once. */
@Composable
internal fun CacheLimitDialog(
    currentMb: Int,
    onDismiss: () -> Unit,
    onSelect: (Int) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("缓存上限") },
        text = {
            Column {
                Text(
                    "上限只对「自动缓存」生效；降低上限会立即清理最久未使用的文件。" +
                        "缓存位于系统缓存目录，存储空间紧张时系统也可能自行清理。",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                LazyColumn(modifier = Modifier.fillMaxWidth().heightIn(max = 280.dp)) {
                    items(CachePrefs.LIMIT_OPTIONS_MB) { limitMb ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelect(limitMb) }
                                .padding(vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            RadioButton(
                                selected = limitMb == currentMb,
                                onClick = { onSelect(limitMb) },
                            )
                            Text(
                                text = CachePrefs.label(limitMb),
                                modifier = Modifier.padding(start = 8.dp),
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("关闭") }
        },
    )
}

/** BV 号 / 链接导入 (the TUI's `b` key). */
