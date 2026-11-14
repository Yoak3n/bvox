package dev.yoa.bvox.ui.components

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp


import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.ArtworkTile
import dev.yoa.bvox.ui.components.BvoxBottomBar
import dev.yoa.bvox.ui.components.BvoxNavItem
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.GradientBackground
import dev.yoa.bvox.ui.screens.library.LibraryScreen
import dev.yoa.bvox.ui.screens.me.MeScreen
import dev.yoa.bvox.ui.screens.nowplaying.NowPlayingScreen
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.login.LoginPanel
import dev.yoa.bvox.ui.components.MiniPlayer
import dev.yoa.bvox.ui.theme.BvoxRadius
import dev.yoa.bvox.core.AppModel
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.ui.screens.discover.DiscoverScreen


@Composable
internal fun MiniPlayer(
    state: Snapshot,
    cover: ImageBitmap?,
    onOpen: () -> Unit,
) {
    val item = state.nowPlaying ?: return
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, bottom = 6.dp),
    ) {
        GlassCard(
            onClick = onOpen,
            shape = RoundedCornerShape(20.dp),
            contentPadding = PaddingValues(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (cover != null) {
                    Image(
                        bitmap = cover,
                        contentDescription = "封面",
                        modifier = Modifier
                            .size(44.dp)
                            .clip(RoundedCornerShape(BvoxRadius.Sm)),
                        contentScale = ContentScale.Crop,
                    )
                } else {
                    ArtworkTile(size = 44.dp, shape = RoundedCornerShape(BvoxRadius.Sm))
                }
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(horizontal = 10.dp),
                ) {
                    Text(
                        text = item.title,
                        style = MaterialTheme.typography.titleSmall,
                        color = BvoxColors.TextPrimary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = item.author,
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                IconButton(onClick = { CoreHolder.core.togglePause() }) {
                    Icon(
                        imageVector = if (state.paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause,
                        contentDescription = if (state.paused) "播放" else "暂停",
                        tint = BvoxColors.TextPrimary,
                    )
                }
                IconButton(onClick = { CoreHolder.core.nextTrack() }) {
                    Icon(
                        imageVector = Icons.Rounded.SkipNext,
                        contentDescription = "下一首",
                        tint = BvoxColors.TextPrimary,
                    )
                }
            }
            if (state.duration > 0) {
                LinearProgressIndicator(
                    progress = { (state.position / state.duration).toFloat().coerceIn(0f, 1f) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 8.dp)
                        .height(2.dp)
                        .clip(RoundedCornerShape(1.dp)),
                    color = BvoxColors.BrandPrimary,
                    trackColor = BvoxColors.Hairline,
                )
            }
        }
    }
}
