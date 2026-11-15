package dev.yoa.bvox.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxRadius

@Composable
internal fun ArtworkTile(
    size: Dp,
    modifier: Modifier = Modifier,
    shape: Shape = RoundedCornerShape(BvoxRadius.Md),
    icon: ImageVector = Icons.Rounded.MusicNote,
) {
    Box(
        modifier = modifier
            .size(size)
            .clip(shape)
            .background(BvoxColors.Accent.copy(alpha = 0.5f)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = Color.White.copy(alpha = 0.9f),
            modifier = Modifier.size(size * 0.42f),
        )
    }
}

/** Cover thumbnail for a track row, with the flat tile as loading/fallback art. */
@Composable
internal fun TrackArtwork(
    url: String?,
    size: Dp,
    modifier: Modifier = Modifier,
    shape: Shape = RoundedCornerShape(BvoxRadius.Sm),
) {
    val thumbnail = remember(url) { listThumbnailUrl(url) }
    val image = rememberRemoteImage(thumbnail)
    if (image == null) {
        ArtworkTile(size = size, modifier = modifier, shape = shape)
    } else {
        Image(
            bitmap = image,
            contentDescription = null,
            modifier = modifier
                .size(size)
                .clip(shape),
            contentScale = ContentScale.Crop,
        )
    }
}
