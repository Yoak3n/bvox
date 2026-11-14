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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp


import dev.yoa.bvox.core.AudioQuality
import dev.yoa.bvox.core.CacheMode
import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.core.PageSummary
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.prefs.CachePrefs


@Composable
fun PagePickerDialog(
    videoTitle: String,
    pages: List<PageSummary>,
    currentPage: Int,
    onDismiss: () -> Unit,
    onPick: (Int) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("切换分P") },
        text = {
            Column {
                Text(
                    videoTitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 360.dp),
                ) {
                    itemsIndexed(pages) { index, page ->
                        val current = index == currentPage
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onPick(index) }
                                .padding(vertical = 8.dp, horizontal = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = page.title,
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontWeight = if (current) FontWeight.SemiBold else FontWeight.Normal,
                                    color = if (current) {
                                        MaterialTheme.colorScheme.primary
                                    } else {
                                        MaterialTheme.colorScheme.onSurface
                                    },
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    text = "P${index + 1} · ${page.duration}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            if (current) {
                                Icon(
                                    imageVector = Icons.Rounded.Check,
                                    contentDescription = "正在播放",
                                    tint = MaterialTheme.colorScheme.primary,
                                )
                            }
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

fun audioQualityLabel(quality: AudioQuality): String = when (quality) {
    AudioQuality.AUTO -> "自动（推荐）"
    AudioQuality.AAC -> "AAC 192K"
    AudioQuality.DOLBY -> "Dolby Atmos"
    AudioQuality.HI_RES -> "Hi-Res 无损"
}

private fun audioQualityHint(quality: AudioQuality, vip: Boolean): String = when (quality) {
    AudioQuality.AUTO ->
        if (vip) "大会员优先 Hi-Res / Dolby，否则 192K AAC"
        else "未开通大会员，最高 192K AAC"
    AudioQuality.AAC -> "始终使用 AAC（最高 192K）"
    AudioQuality.DOLBY ->
        if (vip) "优先 Dolby，不可用时回退 AAC"
        else "需要大会员；无权益时回退 AAC"
    AudioQuality.HI_RES ->
        if (vip) "优先无损 FLAC，不可用时回退 AAC"
        else "需要大会员；无权益时回退 AAC"
}

/** Audio track preference; takes effect on the next track load. */
@Composable
fun AudioQualityDialog(
    current: AudioQuality,
    vip: Boolean,
    onDismiss: () -> Unit,
    onSelect: (AudioQuality) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("音质偏好") },
        text = {
            Column {
                Text(
                    audioQualityHint(current, vip),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                AudioQuality.values().forEach { quality ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(quality) }
                            .padding(vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(
                            selected = current == quality,
                            onClick = { onSelect(quality) },
                        )
                        Column(Modifier.padding(start = 8.dp)) {
                            Text(audioQualityLabel(quality))
                            Text(
                                audioQualityHint(quality, vip),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
                Spacer(Modifier.height(8.dp))
                Text(
                    "切歌后生效；在播放页点刷新可立即生效。",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("关闭") }
        },
    )
}

/** What gets cached: audio included, small files only, or nothing. */
