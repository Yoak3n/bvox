package dev.yoa.bvox.ui.screens.library

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.MusicOff
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem
import dev.yoa.bvox.ui.components.ArtworkTile
import dev.yoa.bvox.ui.components.CircleIconButton
import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.PillButton
import dev.yoa.bvox.ui.components.PlayButton
import dev.yoa.bvox.ui.components.ScreenHeader
import dev.yoa.bvox.ui.components.StatusLine
import dev.yoa.bvox.ui.components.TrackRow
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxRadius
import dev.yoa.bvox.core.CoreHolder


@Composable
internal fun LibraryHome(
    state: Snapshot,
    cover: ImageBitmap?,
    onRefresh: () -> Unit,
    onCreate: () -> Unit,
    onLogin: () -> Unit,
    onOpen: (FavoriteFolder) -> Unit,
    onRename: (FavoriteFolder) -> Unit,
    onDelete: (FavoriteFolder) -> Unit,
    onToggleHidden: (FavoriteFolder, Boolean) -> Unit,
    onToggleShowHidden: () -> Unit,
    onOpenPlayer: () -> Unit,
) {
    val hiddenIds = state.hiddenPlaylistIds.toSet()
    val playlists = if (state.showHidden) {
        state.playlists
    } else {
        state.playlists.filter { it.id !in hiddenIds }
    }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            ScreenHeader(
                kicker = "LIBRARY",
                title = "我的歌单",
                subtitle = "B 站收藏夹就是你的歌单",
                trailing = {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (hiddenIds.isNotEmpty()) {
                            CircleIconButton(
                                icon = if (state.showHidden) {
                                    Icons.Rounded.VisibilityOff
                                } else {
                                    Icons.Rounded.Visibility
                                },
                                contentDescription = if (state.showHidden) {
                                    "收起隐藏歌单"
                                } else {
                                    "显示隐藏歌单"
                                },
                                onClick = onToggleShowHidden,
                            )
                        }
                        CircleIconButton(Icons.Rounded.Refresh, "刷新歌单列表", onRefresh)
                        CircleIconButton(Icons.Rounded.Add, "新建歌单", onCreate)
                    }
                },
            )
        }

        if (state.hasMedia && state.nowPlaying != null) {
            item {
                NowPlayingHero(state, cover, onOpenPlayer)
            }
        }

        item {
            StatusLine(status = state.status, loading = state.loading, error = state.error)
        }

        if (playlists.isEmpty()) {
            item {
                if (state.playlistsLoading) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(160.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator(color = BvoxColors.BrandPrimary)
                    }
                } else if (state.user == null && !state.hasCookie) {
                    EmptyState(
                        icon = Icons.AutoMirrored.Rounded.QueueMusic,
                        title = "登录后查看收藏夹",
                        message = "B 站收藏夹会作为歌单出现在这里",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 32.dp),
                        action = {
                            PillButton("登录 B 站", onClick = onLogin)
                        },
                    )
                } else if (state.user == null) {
                    // A cookie is saved but the nav probe failed (usually
                    // offline), and there is no cached folder list yet.
                    EmptyState(
                        icon = Icons.AutoMirrored.Rounded.QueueMusic,
                        title = "离线，暂无缓存的歌单",
                        message = "联网后刷新会重新加载收藏夹",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 32.dp),
                        action = {
                            PillButton("刷新", onClick = onRefresh)
                        },
                    )
                } else if (state.playlists.isNotEmpty()) {
                    EmptyState(
                        icon = Icons.Rounded.VisibilityOff,
                        title = "所有歌单都已隐藏",
                        message = "隐藏的歌单不会出现在列表里",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 32.dp),
                        action = {
                            PillButton(
                                text = "显示隐藏歌单",
                                onClick = onToggleShowHidden,
                            )
                        },
                    )
                } else {
                    EmptyState(
                        icon = Icons.AutoMirrored.Rounded.QueueMusic,
                        title = "还没有歌单",
                        message = "新建一个歌单，把喜欢的歌收进来",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 32.dp),
                        action = {
                            PillButton("新建歌单", onClick = onCreate)
                        },
                    )
                }
            }
        } else {
            item {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "全部歌单",
                        style = MaterialTheme.typography.titleMedium,
                        color = BvoxColors.TextPrimary,
                    )
                    Spacer(modifier = Modifier.weight(1f))
                    Text(
                        text = if (state.showHidden && hiddenIds.isNotEmpty()) {
                            "${playlists.size} 个 · 含隐藏"
                        } else {
                            "${playlists.size} 个"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                    )
                }
            }
            items(playlists, key = { it.id }) { folder ->
                PlaylistCard(
                    folder = folder,
                    hidden = folder.id in hiddenIds,
                    onOpen = { onOpen(folder) },
                    onRename = { onRename(folder) },
                    onDelete = { onDelete(folder) },
                    onToggleHidden = { onToggleHidden(folder, folder.id !in hiddenIds) },
                )
            }
        }
    }
}
