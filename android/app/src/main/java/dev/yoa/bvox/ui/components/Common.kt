package dev.yoa.bvox.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DownloadForOffline
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxDimens
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius
import dev.yoa.bvox.ui.theme.BvoxSpacing

/** Vertical stage gradient with pink/violet spotlights in the corners. */
@Composable
fun GradientBackground(
    modifier: Modifier = Modifier,
    content: @Composable BoxScope.() -> Unit,
) {
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(BvoxColors.BackgroundGradient))
            .drawBehind {
                drawRect(
                    Brush.radialGradient(
                        colors = listOf(
                            BvoxColors.GlowPrimary.copy(alpha = 0.18f),
                            Color.Transparent,
                        ),
                        center = Offset(size.width * 0.92f, size.height * 0.02f),
                        radius = size.width * 0.95f,
                    )
                )
                drawRect(
                    Brush.radialGradient(
                        colors = listOf(
                            BvoxColors.GlowSecondary.copy(alpha = 0.15f),
                            Color.Transparent,
                        ),
                        center = Offset(size.width * 0.02f, size.height * 0.86f),
                        radius = size.width * 1.1f,
                    )
                )
            },
        content = content,
    )
}

/** Card entrance used across lists: a short rise + fade on first composition. */
@Composable
fun Modifier.riseIn(enabled: Boolean = true): Modifier {
    if (!enabled) return this
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { visible = true }
    val offset by animateFloatAsState(
        targetValue = if (visible) 0f else 10f,
        animationSpec = tween(BvoxMotion.CardRiseMillis, easing = BvoxMotion.EaseOut),
        label = "card_rise",
    )
    val alpha by animateFloatAsState(
        targetValue = if (visible) 1f else 0f,
        animationSpec = tween(BvoxMotion.CardRiseMillis, easing = BvoxMotion.EaseOut),
        label = "card_alpha",
    )
    return graphicsLayer {
        translationY = offset
        this.alpha = alpha
    }
}

@Composable
fun GlassCard(
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(BvoxSpacing.Lg),
    shape: Shape = RoundedCornerShape(BvoxRadius.Lg),
    animated: Boolean = true,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    var surface = modifier
        .fillMaxWidth()
        .riseIn(animated)
        .clip(shape)
        .background(BvoxColors.Card)
        .border(BvoxDimens.Hairline, BvoxColors.Hairline, shape)
    if (onClick != null) {
        surface = surface.clickable(onClick = onClick)
    }
    Column(
        modifier = surface.padding(contentPadding),
        content = content,
    )
}

/** Flat accent tile shown where a cover image is not (yet) available. */
@Composable
fun ArtworkTile(
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
fun TrackArtwork(
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

@Composable
fun CircleIconButton(
    icon: ImageVector,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 40.dp,
    tint: Color = BvoxColors.TextSecondary,
) {
    Box(
        modifier = modifier
            .size(size)
            .clip(CircleShape)
            .background(BvoxColors.Card)
            .border(BvoxDimens.Hairline, BvoxColors.Hairline, CircleShape)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = contentDescription,
            tint = tint,
            modifier = Modifier.size(size * 0.5f),
        )
    }
}

/** Solid pink pill; the main affirmative action on a screen. */
@Composable
fun PillButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
) {
    Row(
        modifier = modifier
            .height(40.dp)
            .clip(RoundedCornerShape(BvoxRadius.Pill))
            .background(BvoxColors.Accent)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier.size(18.dp),
            )
        }
        Text(
            text = text,
            color = Color.White,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
        )
    }
}

@Composable
fun PlayButton(
    paused: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = BvoxDimens.PlayButtonHero,
) {
    Box(
        modifier = modifier
            .size(size)
            .shadow(16.dp, CircleShape, spotColor = BvoxColors.GlowPrimary.copy(alpha = 0.55f))
            .clip(CircleShape)
            .background(BvoxColors.Accent)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause,
            contentDescription = if (paused) "播放" else "暂停",
            tint = Color.White,
            modifier = Modifier.size(size * 0.48f),
        )
    }
}

/** Kicker + big title + subtitle, matching the reference project's headers. */
@Composable
fun ScreenHeader(
    kicker: String,
    title: String,
    subtitle: String? = null,
    modifier: Modifier = Modifier,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = kicker,
                    style = MaterialTheme.typography.labelMedium,
                    color = BvoxColors.BrandPrimary,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.displaySmall,
                    color = BvoxColors.TextPrimary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            trailing()
        }
        if (subtitle != null) {
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = BvoxColors.TextTertiary,
                modifier = Modifier.padding(top = 2.dp),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
fun StatusLine(
    status: String,
    loading: Boolean,
    error: Boolean,
    modifier: Modifier = Modifier,
) {
    // The core keeps its last status forever; these confirmations duplicate
    // what the screen already shows (folder count / opened folder title) or
    // belong to another tab (search result counts).
    val idle = status.matches(Regex("^歌单 \\d+ 个$")) ||
        status.startsWith("已打开") ||
        status.startsWith("找到 ") ||
        status == "没有搜索到结果"
    val text = if (idle) "" else status
    AnimatedVisibility(
        visible = loading || text.isNotBlank(),
        enter = fadeIn(),
        exit = fadeOut(),
        modifier = modifier,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (loading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(12.dp),
                    strokeWidth = 1.5.dp,
                    color = BvoxColors.BrandPrimary,
                )
            }
            Text(
                text = text,
                style = MaterialTheme.typography.bodySmall,
                color = if (error) BvoxColors.StateError else BvoxColors.TextTertiary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
fun EmptyState(
    icon: ImageVector,
    title: String,
    message: String,
    modifier: Modifier = Modifier,
    action: (@Composable () -> Unit)? = null,
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            modifier = Modifier
                .size(76.dp)
                .clip(CircleShape)
                .background(BvoxColors.Card)
                .border(BvoxDimens.Hairline, BvoxColors.Hairline, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = BvoxColors.BrandPrimary,
                modifier = Modifier.size(34.dp),
            )
        }
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = BvoxColors.TextPrimary,
            modifier = Modifier.padding(top = 18.dp),
        )
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = BvoxColors.TextTertiary,
            modifier = Modifier.padding(top = 6.dp, start = 24.dp, end = 24.dp),
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        if (action != null) {
            Spacer(modifier = Modifier.height(16.dp))
            action()
        }
    }
}

@Composable
fun InfoChip(text: String, modifier: Modifier = Modifier, onClick: (() -> Unit)? = null) {
    // Clickable chips (e.g. the 分P counter opening the page picker) keep the
    // same look; a null onClick renders the plain static chip.
    val surface = if (onClick != null) modifier.clickable(onClick = onClick) else modifier
    Surface(
        modifier = surface,
        shape = RoundedCornerShape(BvoxRadius.Pill),
        color = BvoxColors.CardStrong,
        border = androidx.compose.foundation.BorderStroke(BvoxDimens.Hairline, BvoxColors.Hairline),
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
            style = MaterialTheme.typography.labelSmall,
            color = BvoxColors.TextSecondary,
            maxLines = 1,
        )
    }
}

/** One row of the queue/library list, highlighted while it is the current track. */
@Composable
fun TrackRow(
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

data class BvoxNavItem(
    val label: String,
    val icon: ImageVector,
)

/** Semi-transparent attached bar with a rounded animated selection pill. */
@Composable
fun BvoxBottomBar(
    items: List<BvoxNavItem>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(BvoxColors.BottomBar)
            .navigationBarsPadding(),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(BvoxDimens.Hairline)
                .background(BvoxColors.Hairline),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    horizontal = BvoxDimens.BottomNavBarPaddingH,
                    vertical = BvoxDimens.BottomNavBarPaddingV,
                ),
            horizontalArrangement = Arrangement.spacedBy(BvoxDimens.BottomNavItemGap),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            items.forEachIndexed { index, item ->
                BottomBarItem(
                    item = item,
                    selected = selectedIndex == index,
                    modifier = Modifier.weight(1f),
                    onClick = { onSelect(index) },
                )
            }
        }
    }
}

@Composable
private fun BottomBarItem(
    item: BvoxNavItem,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val backgroundColor by animateColorAsState(
        targetValue = if (selected) BvoxColors.BrandPrimary.copy(alpha = 0.14f) else Color.Transparent,
        animationSpec = tween(BvoxMotion.BottomNavMillis),
        label = "nav_bg",
    )
    val contentColor by animateColorAsState(
        targetValue = if (selected) BvoxColors.BrandPrimary else BvoxColors.TextSecondary,
        animationSpec = tween(BvoxMotion.BottomNavMillis),
        label = "nav_color",
    )
    val iconScale by animateFloatAsState(
        targetValue = if (selected) BvoxDimens.BottomNavIconSelectedScale else 1f,
        animationSpec = tween(BvoxMotion.BottomNavMillis),
        label = "nav_scale",
    )
    val shape = RoundedCornerShape(BvoxRadius.Md)

    Box(
        modifier = modifier
            .height(BvoxDimens.BottomNavItemHeight)
            .clip(shape)
            .background(backgroundColor)
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                onClick = onClick,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Icon(
                imageVector = item.icon,
                contentDescription = item.label,
                tint = contentColor,
                modifier = Modifier
                    .size(BvoxDimens.BottomNavIconSize)
                    .graphicsLayer {
                        scaleX = iconScale
                        scaleY = iconScale
                    },
            )
            Text(
                text = item.label,
                color = contentColor,
                style = MaterialTheme.typography.labelSmall,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                maxLines = 1,
            )
        }
    }
}
