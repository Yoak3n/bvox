package dev.yoa.bvox.ui.screens.nowplaying

import androidx.compose.animation.core.tween
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion

@Composable
internal fun LyricsView(state: Snapshot, modifier: Modifier = Modifier) {
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
