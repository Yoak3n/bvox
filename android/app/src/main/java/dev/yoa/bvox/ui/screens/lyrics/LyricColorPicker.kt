package dev.yoa.bvox.ui.screens.lyrics

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.ui.theme.BvoxColors

/** Text color presets for the floating lyric window. */
private val LyricColorChoices = listOf(
    Color.White,
    Color(0xFFFB7299),
    Color(0xFF8B7CFF),
    Color(0xFF4DD0E1),
    Color(0xFFFFD54F),
    Color(0xFF9AE66E),
)

/** Color swatch row; stores the value as ARGB so it round-trips through prefs. */
@Composable
internal fun LyricColorPicker(selectedArgb: Int, onSelect: (Int) -> Unit) {
    Text(
        text = "颜色",
        style = MaterialTheme.typography.labelMedium,
        color = BvoxColors.TextSecondary,
    )
    Row(
        modifier = Modifier.padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        LyricColorChoices.forEach { color ->
            val argb = color.toArgb()
            val selected = argb == selectedArgb
            Box(
                modifier = Modifier
                    .size(28.dp)
                    .clip(CircleShape)
                    .background(color)
                    .border(
                        width = if (selected) 3.dp else 1.dp,
                        color = if (selected) {
                            BvoxColors.TextPrimary
                        } else {
                            Color.White.copy(alpha = 0.25f)
                        },
                        shape = CircleShape,
                    )
                    .clickable { onSelect(argb) },
            )
        }
    }
}
