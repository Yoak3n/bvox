package dev.yoa.bvox.ui.screens.library

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.ArtworkTile
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.PlayButton
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
