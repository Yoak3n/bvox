package dev.yoa.bvox.ui.screens.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.MusicOff
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem
import dev.yoa.bvox.ui.components.CircleIconButton
import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.ui.components.PillButton
import dev.yoa.bvox.ui.components.StatusLine
import dev.yoa.bvox.ui.components.TrackRow
import dev.yoa.bvox.ui.theme.BvoxColors

@Composable
internal fun PlaylistDetail(
    state: Snapshot,
    folder: FavoriteFolder,
    onBack: () -> Unit,
    onPlay: (Int) -> Unit,
    onAddToPlaylist: (VideoItem) -> Unit,
    onRemove: (FavoriteFolder, VideoItem) -> Unit,
) {
    val readOnly = state.currentPlaylistPublic
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 6.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onBack) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Rounded.ArrowBack,
                        contentDescription = "返回歌单",
                        tint = BvoxColors.TextPrimary,
                    )
                }
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = folder.title,
                        style = MaterialTheme.typography.titleLarge,
                        color = BvoxColors.TextPrimary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = if (readOnly) {
                            "公开收藏夹 · ${state.results.size} 首 · 只读"
                        } else {
                            "${state.results.size} 首"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                    )
                }
                if (state.results.isNotEmpty()) {
                    PillButton(text = "播放全部", onClick = { onPlay(0) })
                }
            }
        }

        item {
            StatusLine(status = state.status, loading = state.loading, error = state.error)
        }

        if (state.results.isEmpty()) {
            item {
                if (state.loading) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(160.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator(color = BvoxColors.BrandPrimary)
                    }
                } else {
                    EmptyState(
                        icon = Icons.Rounded.MusicOff,
                        title = if (readOnly) {
                            "这个公开收藏夹没有可播放的视频"
                        } else {
                            "这个歌单还没有歌曲"
                        },
                        message = if (readOnly) {
                            "确认链接指向的是公开收藏夹，且里面有视频"
                        } else {
                            "去「发现」搜索，或用 BV 号导入后加入歌单"
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 32.dp),
                    )
                }
            }
        } else {
            itemsIndexed(
                items = state.results,
                key = { index, item -> "${item.bvid}-$index" },
            ) { index, item ->
                TrackRow(
                    number = "${index + 1}",
                    title = item.title,
                    subtitle = "${item.author} · ${item.duration}",
                    current = state.current?.toInt() == index,
                    cached = state.cachedBvids.contains(item.bvid),
                    artworkUrl = item.pic,
                    onClick = { onPlay(index) },
                    trailing = {
                        CircleIconButton(
                            icon = Icons.Rounded.Add,
                            contentDescription = "加入歌单",
                            onClick = { onAddToPlaylist(item) },
                            size = 32.dp,
                            tint = BvoxColors.TextTertiary,
                        )
                        if (!readOnly) {
                            CircleIconButton(
                                icon = Icons.Rounded.Clear,
                                contentDescription = "从歌单移除",
                                onClick = { onRemove(folder, item) },
                                size = 32.dp,
                                tint = BvoxColors.TextTertiary,
                            )
                        }
                    },
                )
            }
        }
    }
}
