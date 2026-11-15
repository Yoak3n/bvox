package dev.yoa.bvox.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * "Night Vinyl" palette: an ink-dark stage with Bilibili pink and violet
 * stage lights. Cards are translucent white so the gradient bleeds through.
 */
internal object BvoxColors {
    /** Fixed dark presentation; the light set stays for dialogs/WebView shells. */
    var isDark: Boolean = true

    private fun pick(dark: Color, light: Color): Color = if (isDark) dark else light

    val BgApp get() = pick(Color(0xFF0A0B12), Color(0xFFF8F6FB))
    val BgElevated get() = pick(Color(0xFF12141F), Color(0xFFFFFFFF))
    val BgGradientTop get() = pick(Color(0xFF141024), Color(0xFFFDF6FA))
    val BgGradientMiddle get() = pick(Color(0xFF0B0C14), Color(0xFFF7F5FB))
    val BgGradientBottom get() = pick(Color(0xFF101826), Color(0xFFF2F4FA))
    val BottomBar get() = pick(Color(0xF210121A), Color(0xF2FFFFFF))

    val Card get() = pick(Color(0x14FFFFFF), Color(0xB8FFFFFF))
    val CardStrong get() = pick(Color(0x1FFFFFFF), Color(0xE8FFFFFF))
    val Hairline get() = pick(Color(0x1AFFFFFF), Color(0x14000000))
    val HairlineStrong get() = pick(Color(0x2AFFFFFF), Color(0x24000000))

    val TextPrimary get() = pick(Color(0xFFF3F5FA), Color(0xFF16121C))
    val TextSecondary get() = pick(Color(0xFFA8AFC2), Color(0xFF5E5A6B))
    val TextTertiary get() = pick(Color(0xFF747C90), Color(0xFF8C8799))

    /** Pink used for highlights/icons; lifted for contrast on dark. */
    val BrandPrimary get() = pick(Color(0xFFFF8FB0), Color(0xFFE64C7A))
    val BrandPrimarySoft get() = pick(Color(0x24FB7299), Color(0xFFFFE3EC))
    val BrandSecondary get() = pick(Color(0xFFB7A6FF), Color(0xFF6D5AE6))
    val BrandTertiary get() = pick(Color(0xFF5EEAD4), Color(0xFF0D9488))

    /**
     * Solid single-hue accent for large action surfaces and placeholders.
     * Mixing pink into violet across a wide surface reads muddy, so these
     * use pink alone.
     */
    val Accent get() = Color(0xFFFB7299)

    val GlowPrimary get() = Color(0xFFFB7299)
    val GlowSecondary get() = Color(0xFF8B7CFF)

    val StateSuccess get() = pick(Color(0xFF4ADE80), Color(0xFF17A05E))
    val StateWarning get() = pick(Color(0xFFFBBF24), Color(0xFFB45309))
    val StateError get() = pick(Color(0xFFFF6B81), Color(0xFFD92D4B))

    val BackgroundGradient get() = listOf(BgGradientTop, BgGradientMiddle, BgGradientBottom)
}

internal object BvoxRadius {
    // Dp, not Int: RoundedCornerShape(Int) is a percentage and 999 would crash.
    val Sm = 14.dp
    val Md = 18.dp
    val Lg = 24.dp
    val Xl = 30.dp
    val Pill = 999.dp
}

internal object BvoxSpacing {
    val Xs = 4.dp
    val Sm = 8.dp
    val Md = 12.dp
    val Lg = 16.dp
    val Xl = 20.dp
    val Xxl = 24.dp
}

internal object BvoxDimens {
    val PageHorizontal = 20.dp
    val Hairline = 1.dp

    val BottomNavItemHeight = 56.dp
    val BottomNavItemGap = 6.dp
    val BottomNavBarPaddingH = 12.dp
    val BottomNavBarPaddingV = 4.dp
    val BottomNavIconSize = 21.dp
    val BottomNavIconSelectedScale = 1.05f

    val MiniPlayerArtwork = 44.dp
    val TrackArtwork = 44.dp
    val HeroArtwork = 60.dp
    val PlayButtonLarge = 76.dp
    val PlayButtonHero = 46.dp
    val ActionIcon = 20.dp
}

internal object BvoxMotion {
    const val PageTransitionMillis = 180
    const val PageFadeOutMillis = 90
    const val BottomNavMillis = 140
    const val CardRiseMillis = 180
    const val TrackHighlightMillis = 200
    const val ArtworkFloatMillis = 1700
    val ArtworkFloatDistance = 2.6.dp

    val EaseOut = CubicBezierEasing(0.22f, 1f, 0.36f, 1f)
}

internal val BvoxTypography = Typography(
    displaySmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 32.sp,
        lineHeight = 38.sp,
        letterSpacing = (-0.4).sp,
    ),
    headlineSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 24.sp,
        lineHeight = 30.sp,
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 22.sp,
        lineHeight = 28.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 17.sp,
        lineHeight = 24.sp,
    ),
    titleSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 15.sp,
        lineHeight = 21.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 15.sp,
        lineHeight = 22.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 21.sp,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 12.sp,
        lineHeight = 18.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 14.sp,
        lineHeight = 18.sp,
    ),
    labelMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 12.sp,
        lineHeight = 16.sp,
    ),
    labelSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 11.sp,
        lineHeight = 14.sp,
    ),
)

internal val BvoxShapes = Shapes(
    extraSmall = RoundedCornerShape(10.dp),
    small = RoundedCornerShape(14.dp),
    medium = RoundedCornerShape(18.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(30.dp),
)

private val DarkScheme = darkColorScheme(
    primary = Color(0xFFFF8FB0),
    onPrimary = Color(0xFF2A0A13),
    primaryContainer = Color(0xFF4A1F2E),
    onPrimaryContainer = Color(0xFFFFD9E4),
    secondary = Color(0xFFB7A6FF),
    onSecondary = Color(0xFF171036),
    secondaryContainer = Color(0xFF2B2550),
    onSecondaryContainer = Color(0xFFE4DEFF),
    tertiary = Color(0xFF5EEAD4),
    onTertiary = Color(0xFF042F2B),
    background = Color(0xFF0A0B12),
    onBackground = Color(0xFFF3F5FA),
    surface = Color(0xFF12141F),
    onSurface = Color(0xFFF3F5FA),
    surfaceVariant = Color(0xFF1A1D2A),
    onSurfaceVariant = Color(0xFFA8AFC2),
    surfaceContainer = Color(0xFF161925),
    surfaceContainerHigh = Color(0xFF1C2030),
    surfaceContainerHighest = Color(0xFF232838),
    outline = Color(0x33FFFFFF),
    outlineVariant = Color(0x1AFFFFFF),
    error = Color(0xFFFF6B81),
    onError = Color(0xFF2A0A10),
    errorContainer = Color(0xFF43141D),
    onErrorContainer = Color(0xFFFFD9DE),
)

private val LightScheme = lightColorScheme(
    primary = Color(0xFFE64C7A),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFFFE3EC),
    onPrimaryContainer = Color(0xFF3C0A1A),
    secondary = Color(0xFF6D5AE6),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFE8E3FF),
    onSecondaryContainer = Color(0xFF1B1246),
    tertiary = Color(0xFF0D9488),
    onTertiary = Color.White,
    background = Color(0xFFF8F6FB),
    onBackground = Color(0xFF16121C),
    surface = Color.White,
    onSurface = Color(0xFF16121C),
    surfaceVariant = Color(0xFFF0EDF5),
    onSurfaceVariant = Color(0xFF5E5A6B),
    surfaceContainer = Color(0xFFF7F4FA),
    surfaceContainerHigh = Color(0xFFF1EDF7),
    surfaceContainerHighest = Color(0xFFEBE6F2),
    outline = Color(0x33000000),
    outlineVariant = Color(0x14000000),
    error = Color(0xFFD92D4B),
    onError = Color.White,
)

@Composable
internal fun BvoxTheme(
    darkTheme: Boolean = true,
    content: @Composable () -> Unit,
) {
    BvoxColors.isDark = darkTheme
    MaterialTheme(
        colorScheme = if (darkTheme) DarkScheme else LightScheme,
        typography = BvoxTypography,
        shapes = BvoxShapes,
        content = content,
    )
}
