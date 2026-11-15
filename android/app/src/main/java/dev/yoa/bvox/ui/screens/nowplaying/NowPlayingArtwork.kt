package dev.yoa.bvox.ui.screens.nowplaying

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius

@Composable
internal fun NowPlayingArtwork(bitmap: ImageBitmap?, size: Dp) {
    val transition = rememberInfiniteTransition(label = "artwork_float")
    val floatAmount by transition.animateFloat(
        initialValue = -1f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(BvoxMotion.ArtworkFloatMillis, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "artwork_float_y",
    )
    val shape = RoundedCornerShape(BvoxRadius.Xl)
    Box(
        modifier = Modifier
            .size(size)
            .graphicsLayer {
                translationY = floatAmount * BvoxMotion.ArtworkFloatDistance.toPx()
            }
            .shadow(
                elevation = 28.dp,
                shape = shape,
                spotColor = BvoxColors.GlowPrimary,
                ambientColor = BvoxColors.GlowSecondary,
            )
            .clip(shape)
            .background(BvoxColors.Card),
    ) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap,
                contentDescription = "封面",
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        } else {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(BvoxColors.Accent.copy(alpha = 0.55f)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = Icons.Rounded.MusicNote,
                    contentDescription = null,
                    tint = Color.White.copy(alpha = 0.9f),
                    modifier = Modifier.size(size * 0.28f),
                )
            }
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .border(1.dp, Color.White.copy(alpha = 0.14f), shape),
        )
    }
}

@Composable
internal fun CoverBackdrop(bitmap: ImageBitmap) {
    Box(modifier = Modifier.fillMaxSize()) {
        Image(
            bitmap = bitmap,
            contentDescription = null,
            modifier = Modifier
                .fillMaxSize()
                .scale(1.25f)
                .blur(64.dp)
                .alpha(0.35f),
            contentScale = ContentScale.Crop,
        )
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        listOf(
                            BvoxColors.BgApp.copy(alpha = 0.30f),
                            BvoxColors.BgApp.copy(alpha = 0.74f),
                            BvoxColors.BgApp.copy(alpha = 0.96f),
                        )
                    )
                ),
        )
    }
}

/** One chip under the artwork; `action` makes it tappable (分P picker / 音质). */
