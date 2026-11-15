package dev.yoa.bvox.ui.screens.me

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Cached
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.CacheMode
import dev.yoa.bvox.core.CacheUsage
import dev.yoa.bvox.prefs.CachePrefs
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius
import kotlin.math.roundToInt

/**
 * Cache overview: animated size summary, the entry to the cached-song list,
 * the composition bar and legend, plus the strategy/limit chips and the clear
 * action. All numbers share one set of animated values, so a background
 * download or a cleanup reads as one continuous motion instead of a hard jump.
 */
@Composable
internal fun CacheCard(
    usage: CacheUsage?,
    mode: CacheMode,
    limitMb: Int,
    cachedCount: Int,
    onOpenCached: () -> Unit,
    onAdjustMode: () -> Unit,
    onAdjustLimit: () -> Unit,
    onClear: () -> Unit,
) {
    // Audio downloads are capped in auto and audio-only modes; the other modes
    // have no policy target, so the bar only shows composition.
    val limitedMode = mode == CacheMode.AUTO || mode == CacheMode.AUDIO_ONLY
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
    val ratio = when {
        limitedMode && shownLimitMb > 0f -> (usedMb / shownLimitMb).coerceIn(0f, 1f)
        usedMb > 0f -> 1f
        else -> 0f
    }
    val percentColor by animateColorAsState(
        targetValue = when {
            !limitedMode -> BvoxColors.TextTertiary
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
            // The cached-song list is part of this card: same numbers, same
            // place, no second card repeating them.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(BvoxRadius.Sm))
                    .clickable(onClick = onOpenCached)
                    .padding(vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = Icons.Rounded.MusicNote,
                    contentDescription = null,
                    tint = BvoxColors.BrandPrimary,
                    modifier = Modifier.size(15.dp),
                )
                Text(
                    text = if (cachedCount > 0) {
                        "已缓存歌曲 · $cachedCount 首"
                    } else {
                        "已缓存歌曲"
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = BvoxColors.TextSecondary,
                    maxLines = 1,
                    modifier = Modifier.padding(start = 6.dp),
                )
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    text = "查看 ›",
                    style = MaterialTheme.typography.labelSmall,
                    color = BvoxColors.BrandPrimary,
                )
            }
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
                if (limitedMode) {
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
