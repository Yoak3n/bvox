package dev.yoa.bvox.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DownloadForOffline
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxDimens
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius

@Composable
internal fun TrackRow(
    number: String,
    title: String,
    subtitle: String,
    current: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    cached: Boolean = false,
    artworkUrl: String? = null,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    val shape = RoundedCornerShape(BvoxRadius.Md)
    val background by animateColorAsState(
        targetValue = if (current) BvoxColors.BrandPrimary.copy(alpha = 0.13f) else BvoxColors.Card,
        animationSpec = tween(BvoxMotion.TrackHighlightMillis),
        label = "track_bg",
    )
    val borderColor by animateColorAsState(
        targetValue = if (current) BvoxColors.BrandPrimary.copy(alpha = 0.34f) else BvoxColors.Hairline,
        animationSpec = tween(BvoxMotion.TrackHighlightMillis),
        label = "track_border",
    )
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(background)
            .border(BvoxDimens.Hairline, borderColor, shape)
            .clickable(onClick = onClick)
            .padding(start = 12.dp, end = 4.dp, top = 9.dp, bottom = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(modifier = Modifier.width(24.dp), contentAlignment = Alignment.Center) {
            if (current) {
                Icon(
                    imageVector = Icons.Rounded.GraphicEq,
                    contentDescription = "正在播放",
                    tint = BvoxColors.BrandPrimary,
                    modifier = Modifier.size(18.dp),
                )
            } else {
                Text(
                    text = number,
                    style = MaterialTheme.typography.labelMedium,
                    color = BvoxColors.TextTertiary,
                )
            }
        }
        if (!artworkUrl.isNullOrBlank()) {
            TrackArtwork(url = artworkUrl, size = BvoxDimens.TrackArtwork)
        }
        Column(
            modifier = Modifier
                .weight(1f)
                .padding(horizontal = 10.dp),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (current) BvoxColors.BrandPrimary else BvoxColors.TextPrimary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = BvoxColors.TextTertiary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (cached) {
            Icon(
                imageVector = Icons.Rounded.DownloadForOffline,
                contentDescription = "已缓存",
                tint = BvoxColors.BrandTertiary,
                modifier = Modifier
                    .padding(end = 6.dp)
                    .size(16.dp),
            )
        }
        trailing()
    }
}
