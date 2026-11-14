package dev.yoa.bvox.ui.screens

import android.os.Build
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistPlay
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp


import dev.yoa.bvox.core.AudioQuality
import dev.yoa.bvox.core.PlayMode
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem
import dev.yoa.bvox.ui.AudioQualityDialog
import dev.yoa.bvox.ui.PagePickerDialog
import dev.yoa.bvox.ui.audioQualityLabel
import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.InfoChip
import dev.yoa.bvox.ui.components.PlayButton
import dev.yoa.bvox.ui.components.TrackRow
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius
import dev.yoa.bvox.core.AppModel
import dev.yoa.bvox.core.CoreHolder


private fun formatTime(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val minutes = total / 60
    val rest = total % 60
    return "%d:%02d".format(minutes, rest)
}

/** Full player: floating artwork over a blurred cover backdrop, lyrics below. */
@Composable
fun NowPlayingScreen(model: AppModel, modifier: Modifier = Modifier) {
    val snapshot by model.snapshot.collectAsState()
    val cover by model.cover.collectAsState()
    val state = snapshot
    val item = state?.nowPlaying
    val bitmap = cover?.takeIf { it.first == state?.coverKey }?.second

    Box(modifier = modifier.fillMaxSize()) {
        // Modifier.blur needs RenderEffect (API 31); older devices keep the
        // stage gradient instead of showing a sharp cover behind the UI.
        if (bitmap != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            CoverBackdrop(bitmap)
        }
        if (state == null || item == null) {
            EmptyState(
                icon = Icons.Rounded.MusicNote,
                title = "还没有播放歌曲",
                message = "去「发现」或「歌单」挑一首吧",
                modifier = Modifier.fillMaxSize(),
            )
        } else {
            PlayerContent(state, item, bitmap)
        }
    }
}

@Composable
private fun PlayerContent(
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
            Artwork(bitmap, artSize)
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

@Composable
private fun Artwork(bitmap: ImageBitmap?, size: Dp) {
    val transition = rememberInfiniteTransition(label = "artwork_float")
    val floatAmount by transition.animateFloat(
        initialValue = -1f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(BvoxMotion.ArtworkFloatMillis, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "artwork_float_y",
    )
    val shape = RoundedCornerShape(BvoxRadius.Xl)
    Box(
        modifier = Modifier
            .size(size)
            .graphicsLayer {
                translationY = floatAmount * BvoxMotion.ArtworkFloatDistance.toPx()
            }
            .shadow(
                elevation = 28.dp,
                shape = shape,
                spotColor = BvoxColors.GlowPrimary,
                ambientColor = BvoxColors.GlowSecondary,
            )
            .clip(shape)
            .background(BvoxColors.Card),
    ) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap,
                contentDescription = "封面",
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        } else {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(BvoxColors.Accent.copy(alpha = 0.55f)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = Icons.Rounded.MusicNote,
                    contentDescription = null,
                    tint = Color.White.copy(alpha = 0.9f),
                    modifier = Modifier.size(size * 0.28f),
                )
            }
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .border(1.dp, Color.White.copy(alpha = 0.14f), shape),
        )
    }
}

@Composable
private fun CoverBackdrop(bitmap: ImageBitmap) {
    Box(modifier = Modifier.fillMaxSize()) {
        Image(
            bitmap = bitmap,
            contentDescription = null,
            modifier = Modifier
                .fillMaxSize()
                .scale(1.25f)
                .blur(64.dp)
                .alpha(0.35f),
            contentScale = ContentScale.Crop,
        )
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        listOf(
                            BvoxColors.BgApp.copy(alpha = 0.30f),
                            BvoxColors.BgApp.copy(alpha = 0.74f),
                            BvoxColors.BgApp.copy(alpha = 0.96f),
                        )
                    )
                ),
        )
    }
}

/** One chip under the artwork; `action` makes it tappable (分P picker / 音质). */
private data class TrackChip(val text: String, val action: (() -> Unit)? = null)

private fun trackChips(
    state: Snapshot,
    onOpenPages: () -> Unit,
    onOpenQuality: () -> Unit,
): List<TrackChip> {
    val chips = mutableListOf<TrackChip>()
    val page = state.page.toInt()
    val pageCount = state.pageCount.toInt()
    if (pageCount > 1) chips += TrackChip("P${page + 1}/$pageCount", onOpenPages)
    if (state.quality.isNotEmpty()) chips += TrackChip(state.quality, onOpenQuality)
    if (state.qualityPref != AudioQuality.AUTO) {
        chips += TrackChip("音质 ${audioQualityLabel(state.qualityPref)}", onOpenQuality)
    }
    // The 分P title already serves as the track heading, so the chips only
    // carry playback facts (stream, cache).
    if (state.cachedNow) chips += TrackChip("已缓存")
    state.cacheProgress?.let { progress ->
        val downloaded = progress.downloaded.toLong()
        val total = progress.total?.toLong()
        val text = if (total != null && total > 0) {
            "缓存中 ${downloaded * 100 / total}%"
        } else {
            "缓存中 ${downloaded / 1_048_576} MB"
        }
        chips += TrackChip(text)
    }
    return chips
}

/** Scrolling lyrics with the active line lifted and tinted. */
@Composable
private fun LyricsView(state: Snapshot, modifier: Modifier = Modifier) {
    val listState = rememberLazyListState()
    val time = state.position + 0.2
    var activeIndex = -1
    for ((index, line) in state.lyrics.withIndex()) {
        if (line.from <= time) activeIndex = index else break
    }

    LaunchedEffect(activeIndex) {
        if (activeIndex >= 0) {
            listState.animateScrollToItem(activeIndex)
        }
    }

    LazyColumn(
        state = listState,
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        itemsIndexed(state.lyrics) { index, line ->
            val active = index == activeIndex
            val color by animateColorAsState(
                targetValue = if (active) BvoxColors.BrandPrimary else BvoxColors.TextTertiary,
                animationSpec = tween(BvoxMotion.TrackHighlightMillis),
                label = "lyric_color",
            )
            Text(
                text = line.text,
                style = if (active) {
                    MaterialTheme.typography.titleSmall
                } else {
                    MaterialTheme.typography.bodyMedium
                },
                fontWeight = if (active) FontWeight.SemiBold else FontWeight.Normal,
                color = color,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 5.dp),
            )
        }
    }
}

/** Bottom sheet with the current queue; tapping a row jumps to that track. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun QueueSheet(
    state: Snapshot,
    onDismiss: () -> Unit,
    onPlay: (Int) -> Unit,
) {
    val listState = rememberLazyListState()
    LaunchedEffect(state.current) {
        state.current?.toInt()?.let { index ->
            if (index >= 0) listState.animateScrollToItem(index)
        }
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "播放队列",
                style = MaterialTheme.typography.titleMedium,
                color = BvoxColors.TextPrimary,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = "${state.results.size} 首",
                style = MaterialTheme.typography.bodySmall,
                color = BvoxColors.TextTertiary,
            )
        }
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxWidth(),
            contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            itemsIndexed(state.results, key = { index, item -> "$index-${item.bvid}" }) { index, item ->
                TrackRow(
                    number = "${index + 1}",
                    title = item.title,
                    subtitle = "${item.author} · ${item.duration}",
                    current = state.current?.toInt() == index,
                    artworkUrl = item.pic,
                    onClick = { onPlay(index) },
                )
            }
        }
    }
}

private fun PlayMode.next(): PlayMode = when (this) {
    PlayMode.SEQUENTIAL -> PlayMode.REPEAT_ALL
    PlayMode.REPEAT_ALL -> PlayMode.REPEAT_ONE
    PlayMode.REPEAT_ONE -> PlayMode.SHUFFLE
    PlayMode.SHUFFLE -> PlayMode.SEQUENTIAL
}

private fun playModeLabel(mode: PlayMode): String = when (mode) {
    PlayMode.SEQUENTIAL -> "顺序播放"
    PlayMode.REPEAT_ALL -> "列表循环"
    PlayMode.REPEAT_ONE -> "单曲循环"
    PlayMode.SHUFFLE -> "随机播放"
}

private fun playModeIcon(mode: PlayMode): ImageVector = when (mode) {
    PlayMode.SEQUENTIAL -> Icons.AutoMirrored.Rounded.PlaylistPlay
    PlayMode.REPEAT_ALL -> Icons.Rounded.Repeat
    PlayMode.REPEAT_ONE -> Icons.Rounded.RepeatOne
    PlayMode.SHUFFLE -> Icons.Rounded.Shuffle
}
