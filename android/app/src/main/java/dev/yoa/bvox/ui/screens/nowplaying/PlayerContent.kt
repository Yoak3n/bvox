package dev.yoa.bvox.ui.screens.nowplaying

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.InfoChip
import dev.yoa.bvox.ui.components.PlayButton
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.ui.dialogs.PagePickerDialog
import dev.yoa.bvox.ui.dialogs.AudioQualityDialog

internal fun formatTime(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val minutes = total / 60
    val rest = total % 60
    return "%d:%02d".format(minutes, rest)
}

/** Full player: floating artwork over a blurred cover backdrop, lyrics below. */

@Composable
internal fun PlayerContent(
    state: Snapshot,
    item: VideoItem,
    bitmap: ImageBitmap?,
) {
    var showQualityDialog by remember { mutableStateOf(false) }
    var showPagePicker by remember { mutableStateOf(false) }
    var showQueue by remember { mutableStateOf(false) }
    var showMoreMenu by remember { mutableStateOf(false) }
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp)
            .padding(top = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box {
                IconButton(onClick = { showMoreMenu = true }) {
                    Icon(
                        imageVector = Icons.Rounded.MoreVert,
                        contentDescription = "更多操作",
                        tint = BvoxColors.TextTertiary,
                    )
                }
                DropdownMenu(
                    expanded = showMoreMenu,
                    onDismissRequest = { showMoreMenu = false },
                ) {
                    DropdownMenuItem(
                        text = { Text("重新获取播放地址") },
                        leadingIcon = {
                            Icon(
                                imageVector = Icons.Rounded.Refresh,
                                contentDescription = null,
                            )
                        },
                        onClick = {
                            showMoreMenu = false
                            CoreHolder.core.retryCurrent()
                        },
                    )
                }
            }
        }

        BoxWithConstraints(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1.4f)
                .padding(vertical = 8.dp),
            contentAlignment = Alignment.Center,
        ) {
            val artSize = minOf(maxWidth * 0.78f, maxHeight)
            NowPlayingArtwork(bitmap, artSize)
        }

        Text(
            text = item.title,
            style = MaterialTheme.typography.titleLarge,
            color = BvoxColors.TextPrimary,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 8.dp),
        )
        Text(
            text = item.author,
            style = MaterialTheme.typography.bodyMedium,
            color = BvoxColors.TextSecondary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 2.dp),
        )

        val chips = trackChips(
            state,
            onOpenPages = { showPagePicker = true },
            onOpenQuality = { showQualityDialog = true },
        )
        if (chips.isNotEmpty()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
            ) {
                chips.forEach { chip -> InfoChip(chip.text, onClick = chip.action) }
            }
        }

        val duration = state.duration.toFloat().coerceAtLeast(1f)
        var dragging by remember { mutableStateOf(false) }
        var dragValue by remember { mutableFloatStateOf(0f) }
        val sliderValue = if (dragging) dragValue else state.position.toFloat().coerceIn(0f, duration)
        Slider(
            value = sliderValue,
            onValueChange = {
                dragging = true
                dragValue = it
            },
            onValueChangeFinished = {
                CoreHolder.core.seekTo(dragValue.toDouble())
                dragging = false
            },
            valueRange = 0f..duration,
            colors = SliderDefaults.colors(
                thumbColor = BvoxColors.BrandPrimary,
                activeTrackColor = BvoxColors.BrandPrimary,
                inactiveTrackColor = BvoxColors.CardStrong,
                activeTickColor = Color.Transparent,
                inactiveTickColor = Color.Transparent,
            ),
            modifier = Modifier.padding(top = 4.dp),
        )
        Row(modifier = Modifier.fillMaxWidth()) {
            Text(
                text = formatTime(sliderValue.toDouble()),
                style = MaterialTheme.typography.labelSmall,
                color = BvoxColors.TextTertiary,
            )
            Text(
                text = formatTime(state.duration),
                style = MaterialTheme.typography.labelSmall,
                color = BvoxColors.TextTertiary,
                modifier = Modifier.weight(1f),
                textAlign = TextAlign.End,
            )
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 6.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(
                onClick = { CoreHolder.core.setPlayMode(state.playMode.next()) },
                modifier = Modifier.size(56.dp),
            ) {
                Icon(
                    imageVector = playModeIcon(state.playMode),
                    contentDescription = playModeLabel(state.playMode),
                    tint = BvoxColors.TextTertiary,
                    modifier = Modifier.size(24.dp),
                )
            }
            IconButton(
                onClick = { CoreHolder.core.previousTrack() },
                modifier = Modifier.size(56.dp),
            ) {
                Icon(
                    imageVector = Icons.Rounded.SkipPrevious,
                    contentDescription = "上一首",
                    tint = BvoxColors.TextPrimary,
                    modifier = Modifier.size(34.dp),
                )
            }
            Box(modifier = Modifier.padding(horizontal = 18.dp)) {
                PlayButton(
                    paused = state.paused,
                    onClick = { CoreHolder.core.togglePause() },
                    size = 72.dp,
                )
            }
            IconButton(
                onClick = { CoreHolder.core.nextTrack() },
                modifier = Modifier.size(56.dp),
            ) {
                Icon(
                    imageVector = Icons.Rounded.SkipNext,
                    contentDescription = "下一首",
                    tint = BvoxColors.TextPrimary,
                    modifier = Modifier.size(34.dp),
                )
            }
            IconButton(
                onClick = { showQueue = true },
                modifier = Modifier.size(56.dp),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Rounded.QueueMusic,
                    contentDescription = "播放队列",
                    tint = BvoxColors.TextTertiary,
                    modifier = Modifier.size(24.dp),
                )
            }
        }

        GlassCard(
            modifier = Modifier
                .fillMaxWidth()
                .weight(0.95f)
                .padding(top = 8.dp),
            contentPadding = PaddingValues(horizontal = 14.dp, vertical = 12.dp),
            animated = false,
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "歌词",
                    style = MaterialTheme.typography.labelMedium,
                    color = BvoxColors.TextTertiary,
                    modifier = Modifier.weight(1f),
                )
                IconButton(
                    onClick = { CoreHolder.core.refreshLyrics() },
                    modifier = Modifier.size(28.dp),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Refresh,
                        contentDescription = "重新获取歌词",
                        tint = BvoxColors.TextTertiary,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
            if (state.lyrics.isEmpty()) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = state.lyricsStatus.ifEmpty { "暂无歌词" },
                        style = MaterialTheme.typography.bodyMedium,
                        color = BvoxColors.TextTertiary,
                        textAlign = TextAlign.Center,
                    )
                }
            } else {
                LyricsView(
                    state = state,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .padding(top = 4.dp),
                )
            }
        }

        if (showQualityDialog) {
            AudioQualityDialog(
                current = state.qualityPref,
                vip = state.vip,
                onDismiss = { showQualityDialog = false },
                onSelect = { quality -> CoreHolder.core.setAudioQuality(quality) },
            )
        }

        if (showPagePicker) {
            PagePickerDialog(
                videoTitle = state.results.getOrNull(state.current?.toInt() ?: -1)?.title
                    ?: item.title,
                pages = state.pages,
                currentPage = state.page.toInt(),
                onDismiss = { showPagePicker = false },
                onPick = { index ->
                    showPagePicker = false
                    CoreHolder.core.playPage(index.toUInt())
                },
            )
        }

        if (showQueue) {
            QueueSheet(
                state = state,
                onDismiss = { showQueue = false },
                onPlay = { index ->
                    CoreHolder.core.play(index.toUInt())
                    showQueue = false
                },
            )
        }
    }
}
