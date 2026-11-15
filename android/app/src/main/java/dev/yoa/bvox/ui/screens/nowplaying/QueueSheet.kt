package dev.yoa.bvox.ui.screens.nowplaying

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.TrackRow
import dev.yoa.bvox.ui.theme.BvoxColors

/** Bottom sheet with the current queue; tapping a row jumps to that track. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun QueueSheet(
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
