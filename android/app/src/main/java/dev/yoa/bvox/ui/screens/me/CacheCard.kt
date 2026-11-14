package dev.yoa.bvox.ui.screens.me

import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Logout
import androidx.compose.material.icons.rounded.Cached
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp



import dev.yoa.bvox.core.CacheMode
import dev.yoa.bvox.core.CacheUsage
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.PillButton
import dev.yoa.bvox.ui.components.ScreenHeader
import dev.yoa.bvox.ui.components.StatusLine
import dev.yoa.bvox.ui.components.rememberRemoteImage
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxDimens
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius
import java.util.Locale
import kotlin.math.roundToInt
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.lyrics.LyricDisplayPrefs
import dev.yoa.bvox.prefs.CachePrefs
import dev.yoa.bvox.ui.screens.cached.CachedTracksScreen
import dev.yoa.bvox.ui.screens.lyrics.LyricSettingsScreen
import dev.yoa.bvox.ui.dialogs.CacheModeDialog
import dev.yoa.bvox.ui.dialogs.CacheLimitDialog


internal const val BYTES_PER_MB = 1024.0 * 1024.0

/** One colored segment of the cache bar; the legend uses the same values. */
internal data class CacheSlice(val label: String, val mb: Float, val color: Color)

/** Animated per-category size in MiB; snapshots glide instead of snapping. */
@Composable
internal fun animatedCacheMb(bytes: ULong?): Float =
    animateFloatAsState(
        targetValue = (bytes ?: 0uL).toDouble().div(BYTES_PER_MB).toFloat(),
        animationSpec = tween(420, easing = BvoxMotion.EaseOut),
        label = "cache_slice_mb",
    ).value

/**
 * Cache overview: animated size summary, composition bar and legend, plus the
 * strategy/limit chips and the clear action. All numbers share one set of
 * animated values, so a background download or a cleanup reads as one
 * continuous motion instead of a hard jump.
 */

@Composable
internal fun CacheCard(
    usage: CacheUsage?,
    mode: CacheMode,
    limitMb: Int,
    onAdjustMode: () -> Unit,
    onAdjustLimit: () -> Unit,
    onClear: () -> Unit,
) {
    val autoMode = mode == CacheMode.AUTO
    val audioMb = animatedCacheMb(usage?.audioBytes)
    val coverMb = animatedCacheMb(usage?.coverBytes)
    val lyricsMb = animatedCacheMb(usage?.lyricsBytes)
    val viewMb = animatedCacheMb(usage?.viewBytes)
    val usedMb = audioMb + coverMb + lyricsMb + viewMb
    // Animate the cap too, so picking 4 GB lets the bar glide down to its new
    // fill level instead of jumping.
    val shownLimitMb by animateFloatAsState(
        targetValue = limitMb.toFloat(),
        animationSpec = tween(420, easing = BvoxMotion.EaseOut),
        label = "cache_limit_mb",
    )
    // Outside auto mode the cap is not a policy target: fill the bar with the
    // composition of whatever is on disk instead.
    val ratio = when {
        autoMode && shownLimitMb > 0f -> (usedMb / shownLimitMb).coerceIn(0f, 1f)
        usedMb > 0f -> 1f
        else -> 0f
    }
    val percentColor by animateColorAsState(
        targetValue = when {
            !autoMode -> BvoxColors.TextTertiary
            ratio >= 0.98f -> BvoxColors.StateError
            ratio >= 0.9f -> BvoxColors.StateWarning
            else -> BvoxColors.TextTertiary
        },
        animationSpec = tween(BvoxMotion.TrackHighlightMillis),
        label = "cache_percent_color",
    )
    val slices = listOf(
        CacheSlice("音频", audioMb, BvoxColors.BrandPrimary),
        CacheSlice("封面", coverMb, BvoxColors.BrandSecondary),
        CacheSlice("歌词", lyricsMb, BvoxColors.BrandTertiary),
        CacheSlice("信息", viewMb, BvoxColors.StateWarning),
    )
    val canClear = usage != null && usage.usedBytes > 0uL

    GlassCard {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(CircleShape)
                        .background(BvoxColors.BrandPrimarySoft),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Cached,
                        contentDescription = null,
                        tint = BvoxColors.BrandPrimary,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(horizontal = 14.dp),
                ) {
                    Text(
                        text = "缓存",
                        style = MaterialTheme.typography.titleSmall,
                        color = BvoxColors.TextPrimary,
                    )
                    when {
                        usage == null -> Text(
                            text = "统计中…",
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                        mode == CacheMode.OFF -> Text(
                            text = "已关闭 · 占用 ${formatBytes(usedMb * BYTES_PER_MB)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                        mode == CacheMode.SMALL_FILES -> Text(
                            text = "仅小文件 · 占用 ${formatBytes(usedMb * BYTES_PER_MB)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                        else -> Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                text = "${formatBytes(usedMb * BYTES_PER_MB)} / " +
                                    formatBytes(limitMb * BYTES_PER_MB),
                                style = MaterialTheme.typography.bodySmall,
                                color = BvoxColors.TextTertiary,
                            )
                            Text(
                                text = " · ${(ratio * 100f).roundToInt()}%",
                                style = MaterialTheme.typography.bodySmall,
                                color = percentColor,
                                fontWeight = if (ratio >= 0.9f) {
                                    FontWeight.SemiBold
                                } else {
                                    FontWeight.Normal
                                },
                            )
                        }
                    }
                }
            }
            // Keep a sliver visible for tiny usage against a big cap.
            val fill = if (usedMb > 0f) ratio.coerceAtLeast(0.006f) else 0f
            CacheUsageBar(
                fill = fill,
                slices = slices,
                modifier = Modifier.padding(top = 12.dp),
            )
            CacheLegend(
                slices = slices,
                modifier = Modifier.padding(top = 10.dp),
            )
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                CacheChip(
                    label = "策略 ${CachePrefs.modeLabel(mode)}",
                    icon = Icons.Rounded.Cached,
                    onClick = onAdjustMode,
                )
                if (autoMode) {
                    Spacer(modifier = Modifier.width(6.dp))
                    CacheChip(
                        label = "上限 ${CachePrefs.label(limitMb)}",
                        icon = Icons.Rounded.Tune,
                        onClick = onAdjustLimit,
                    )
                }
                Spacer(modifier = Modifier.weight(1f))
                TextButton(onClick = onClear, enabled = canClear) {
                    Text(
                        text = "清理",
                        color = if (canClear) {
                            BvoxColors.BrandPrimary
                        } else {
                            BvoxColors.TextTertiary
                        },
                    )
                }
            }
        }
    }
}

/** Small pill that opens one of the cache dialogs; presses squash it slightly. */
@Composable
internal fun CacheChip(
    label: String,
    icon: ImageVector,
    onClick: () -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed) 0.94f else 1f,
        animationSpec = tween(120, easing = BvoxMotion.EaseOut),
        label = "cache_chip_press",
    )
    val shape = RoundedCornerShape(BvoxRadius.Pill)
    Surface(
        modifier = Modifier
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .clip(shape)
            .clickable(
                interactionSource = interaction,
                indication = LocalIndication.current,
                onClick = onClick,
            ),
        shape = shape,
        color = BvoxColors.CardStrong,
        border = BorderStroke(BvoxDimens.Hairline, BvoxColors.Hairline),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 7.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = BvoxColors.BrandPrimary,
                modifier = Modifier.size(15.dp),
            )
            Crossfade(
                targetState = label,
                animationSpec = tween(180),
                label = "cache_chip_label",
            ) { value ->
                Text(
                    text = value,
                    style = MaterialTheme.typography.labelMedium,
                    color = BvoxColors.TextSecondary,
                    maxLines = 1,
                )
            }
            Icon(
                imageVector = Icons.Rounded.ExpandMore,
                contentDescription = null,
                tint = BvoxColors.TextTertiary,
                modifier = Modifier.size(15.dp),
            )
        }
    }
}

/**
 * Composition bar: its length is `used / limit`, and the filled part is split
 * into one color per cache type. Weights come from the animated sizes, so
 * categories grow and shrink continuously.
 */
