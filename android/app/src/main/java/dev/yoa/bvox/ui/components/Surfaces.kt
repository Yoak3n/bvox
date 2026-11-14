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
