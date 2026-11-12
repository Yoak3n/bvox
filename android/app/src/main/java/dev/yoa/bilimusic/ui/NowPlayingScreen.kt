package dev.yoa.bilimusic.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.Image
import dev.yoa.bilimusic.AppModel
import dev.yoa.bilimusic.CoreHolder
import dev.yoa.bilimusic.core.Snapshot

private fun formatTime(seconds: Double): String {
    val total = seconds.toLong().coerceAtLeast(0)
    val m = total / 60
    val s = total % 60
    return "%d:%02d".format(m, s)
}

/** Full-screen player: cover, metadata, seek slider, controls and lyrics. */
@Composable
fun NowPlayingScreen(model: AppModel, onClose: () -> Unit) {
    val snapshot by model.snapshot.collectAsState()
    val cover by model.cover.collectAsState()
    val state = snapshot ?: return

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Top bar
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onClose) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
            }
            Text(
                "正在播放",
                style = MaterialTheme.typography.titleMedium,
            )
        }

        CoverImage(state, cover, Modifier.weight(4f))

        // Metadata
        val item = state.nowPlaying
        Text(
            item?.title ?: "（未播放）",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            maxLines = 2,
        )
        Text(
            item?.author ?: "",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            trackInfoLine(state),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        // Seek slider
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
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(formatTime(sliderValue.toDouble()), style = MaterialTheme.typography.labelSmall)
            Text(formatTime(state.duration), style = MaterialTheme.typography.labelSmall)
        }

        // Controls
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 8.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = { CoreHolder.core.previousTrack() }, modifier = Modifier.size(56.dp)) {
                Icon(Icons.Filled.SkipPrevious, contentDescription = "上一首", modifier = Modifier.size(36.dp))
            }
            IconButton(onClick = { CoreHolder.core.togglePause() }, modifier = Modifier.size(72.dp)) {
                Icon(
                    if (state.paused) Icons.Filled.PlayArrow else Icons.Filled.Pause,
                    contentDescription = if (state.paused) "播放" else "暂停",
                    modifier = Modifier.size(52.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
            }
            IconButton(onClick = { CoreHolder.core.nextTrack() }, modifier = Modifier.size(56.dp)) {
                Icon(Icons.Filled.SkipNext, contentDescription = "下一首", modifier = Modifier.size(36.dp))
            }
            IconButton(onClick = { CoreHolder.core.retryCurrent() }, modifier = Modifier.size(44.dp)) {
                Icon(Icons.Filled.Refresh, contentDescription = "重新获取播放地址")
            }
        }

        // Lyrics
        Box(modifier = Modifier.weight(3f), contentAlignment = Alignment.Center) {
            if (state.lyrics.isEmpty()) {
                Text(
                    state.lyricsStatus.ifEmpty { "暂无歌词" },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                LyricsView(state)
            }
        }
    }
}

@Composable
private fun CoverImage(
    state: Snapshot,
    cover: Pair<String, androidx.compose.ui.graphics.ImageBitmap?>?,
    modifier: Modifier = Modifier,
) {
    val bitmap = cover?.takeIf { it.first == state.coverKey }?.second
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap,
                contentDescription = "封面",
                modifier = Modifier
                    .fillMaxWidth(0.62f)
                    .aspectRatio(1f)
                    .background(
                        MaterialTheme.colorScheme.surfaceVariant,
                        RoundedCornerShape(12.dp),
                    ),
                contentScale = ContentScale.Crop,
            )
        } else {
            Box(
                modifier = Modifier
                    .fillMaxWidth(0.62f)
                    .aspectRatio(1f)
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Text("♪", style = MaterialTheme.typography.displayLarge, color = Color.Gray)
            }
        }
    }
}

private fun trackInfoLine(state: Snapshot): String {
    val page = state.page.toInt()
    val pageCount = state.pageCount.toInt()
    val parts = mutableListOf<String>()
    if (pageCount > 1) parts.add("P${page + 1}/$pageCount")
    if (state.quality.isNotEmpty()) parts.add(state.quality)
    if (state.part.isNotEmpty()) parts.add(state.part)
    if (state.cachedNow) parts.add("已缓存")
    val cacheProgress = state.cacheProgress
    if (cacheProgress != null) {
        val downloaded = cacheProgress.downloaded.toLong()
        val total = cacheProgress.total?.toLong()
        val text = if (total != null && total > 0) {
            "缓存中 ${downloaded * 100 / total}%"
        } else {
            "缓存中 ${downloaded / 1_048_576} MB"
        }
        parts.add(text)
    }
    return parts.joinToString(" · ")
}

/** Scrolling lyrics with the active line highlighted (mirrors App::lyric_cursor). */
@Composable
private fun LyricsView(state: Snapshot) {
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
        modifier = Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        itemsIndexed(state.lyrics) { index, line ->
            val active = index == activeIndex
            Text(
                line.text,
                style = if (active) MaterialTheme.typography.titleMedium else MaterialTheme.typography.bodyMedium,
                fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(vertical = 6.dp),
            )
        }
    }
}
