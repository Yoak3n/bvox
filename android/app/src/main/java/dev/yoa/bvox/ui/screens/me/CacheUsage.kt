package dev.yoa.bvox.ui.screens.me

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import java.util.Locale
import kotlin.math.roundToInt

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

/** Composition bar: its length is `used / limit`, split by cache type. */
@Composable
internal fun CacheUsageBar(
    fill: Float,
    slices: List<CacheSlice>,
    modifier: Modifier = Modifier,
) {
    val totalMb = slices.sumOf { it.mb.toDouble() }.toFloat()
    // Give tiny categories a visible minimum: a purely proportional bar would
    // render 3 KB of metadata as a zero-width sliver next to the audio bytes.
    val floor = totalMb * 0.08f
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(6.dp)
            .clip(RoundedCornerShape(3.dp))
            .background(BvoxColors.Hairline),
    ) {
        if (fill > 0f) {
            Row(
                modifier = Modifier
                    .fillMaxHeight()
                    .fillMaxWidth(fill.coerceIn(0f, 1f)),
            ) {
                slices.forEach { slice ->
                    Box(
                        modifier = Modifier
                            .weight((slice.mb + floor).coerceAtLeast(0.001f))
                            .fillMaxHeight()
                            .background(slice.color),
                    )
                }
            }
        }
    }
}

/** Color legend under the bar: one column per cache type, values on a second line. */
@Composable
internal fun CacheLegend(slices: List<CacheSlice>, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        slices.forEach { slice ->
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        modifier = Modifier
                            .size(6.dp)
                            .clip(CircleShape)
                            .background(slice.color),
                    )
                    Spacer(modifier = Modifier.width(5.dp))
                    Text(
                        text = slice.label,
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                    )
                }
                Text(
                    text = formatBytes(slice.mb * BYTES_PER_MB),
                    style = MaterialTheme.typography.bodySmall,
                    color = BvoxColors.TextSecondary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** Compact size label for cache cards; mirrors the Rust `format_bytes`. */

internal fun formatBytes(bytes: Double): String = when {
    bytes >= 1024.0 * 1024.0 * 1024.0 ->
        String.format(Locale.US, "%.1f GB", bytes / (1024.0 * 1024.0 * 1024.0))
    bytes >= 1024.0 * 1024.0 ->
        String.format(Locale.US, "%.1f MB", bytes / (1024.0 * 1024.0))
    bytes >= 1024.0 ->
        String.format(Locale.US, "%.1f KB", bytes / 1024.0)
    else -> "${bytes.roundToInt()} B"
}
