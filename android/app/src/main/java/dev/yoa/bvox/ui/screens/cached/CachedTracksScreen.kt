package dev.yoa.bvox.ui.screens.cached

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.core.CachedTrack
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.ArtworkTile
import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.ScreenHeader
import dev.yoa.bvox.ui.theme.BvoxColors

/**
 * Offline-only list of finished audio files. Deliberately independent of the
 * favorite folder list: cached songs stay reachable even when the library
 * cannot be loaded (no network, expired cookie, empty cache of playlists).
 */
@Composable
fun CachedTracksScreen(
    state: Snapshot,
    onBack: () -> Unit,
    onPlay: (String) -> Unit,
) {
    BackHandler(onBack = onBack)
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            ScreenHeader(
                kicker = "OFFLINE",
                title = "已缓存歌曲",
                subtitle = "${state.cachedTracks.size} 首 · 断网也能播放",
                trailing = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Rounded.ArrowBack,
                            contentDescription = "返回",
                            tint = BvoxColors.TextSecondary,
                        )
                    }
                },
            )
        }

        if (state.cachedTracks.isEmpty()) {
            item {
                EmptyState(
                    icon = Icons.Rounded.MusicNote,
                    title = "还没有缓存歌曲",
                    message = "播放过的歌会自动缓存；开启「自动缓存」后可用",
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 32.dp),
                )
            }
        } else {
            items(state.cachedTracks, key = { "${it.bvid}:${it.cid}" }) { track ->
                CachedTrackRow(track = track, onPlay = { onPlay(track.bvid) })
            }
        }
    }
}

@Composable
private fun CachedTrackRow(track: CachedTrack, onPlay: () -> Unit) {
    GlassCard(onClick = onPlay, contentPadding = PaddingValues(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            ArtworkTile(size = 48.dp)
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 12.dp),
            ) {
                Text(
                    text = track.title,
                    style = MaterialTheme.typography.bodyLarge,
                    color = BvoxColors.TextPrimary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = buildString {
                        if (track.part.isNotBlank()) {
                            append(track.part)
                            append(" · ")
                        }
                        if (track.author.isNotBlank()) {
                            append(track.author)
                            append(" · ")
                        }
                        append(track.duration)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = BvoxColors.TextTertiary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = track.quality,
                    style = MaterialTheme.typography.labelSmall,
                    color = BvoxColors.BrandPrimary,
                )
            }
            Icon(
                imageVector = Icons.Rounded.PlayArrow,
                contentDescription = "播放",
                tint = BvoxColors.BrandPrimary,
            )
        }
    }
}
