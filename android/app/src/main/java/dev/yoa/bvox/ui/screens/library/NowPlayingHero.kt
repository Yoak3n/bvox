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
internal fun NowPlayingHero(
    state: Snapshot,
    cover: ImageBitmap?,
    onOpen: () -> Unit,
) {
    val item = state.nowPlaying ?: return
    GlassCard(
        onClick = onOpen,
        contentPadding = PaddingValues(14.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (cover != null) {
                Image(
                    bitmap = cover,
                    contentDescription = "封面",
                    modifier = Modifier
                        .size(60.dp)
                        .clip(RoundedCornerShape(BvoxRadius.Md)),
                    contentScale = ContentScale.Crop,
                )
            } else {
                ArtworkTile(size = 60.dp)
            }
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 12.dp),
            ) {
                Text(
                    text = "继续聆听",
                    style = MaterialTheme.typography.labelMedium,
                    color = BvoxColors.BrandPrimary,
                )
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
            PlayButton(
                paused = state.paused,
                onClick = { CoreHolder.core.togglePause() },
                size = 46.dp,
            )
        }
        if (state.duration > 0) {
            LinearProgressIndicator(
                progress = { (state.position / state.duration).toFloat().coerceIn(0f, 1f) },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 12.dp)
                    .height(3.dp)
                    .clip(RoundedCornerShape(2.dp)),
                color = BvoxColors.BrandPrimary,
                trackColor = BvoxColors.Hairline,
            )
        }
    }
}
