package dev.yoa.bvox.ui.screens

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
import dev.yoa.bvox.ui.CacheLimitDialog
import dev.yoa.bvox.ui.CacheModeDialog
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






/** Account and build info. */
@Composable
fun MeScreen(
    state: Snapshot,
    onLogin: () -> Unit,
    onLogout: () -> Unit,
    onPlayCached: (String) -> Unit,
) {
    var showLyricSettings by rememberSaveable { mutableStateOf(false) }
    if (showLyricSettings) {
        LyricSettingsScreen(onBack = { showLyricSettings = false })
        return
    }
    var showCachedTracks by rememberSaveable { mutableStateOf(false) }
    if (showCachedTracks) {
        CachedTracksScreen(
            state = state,
            onBack = { showCachedTracks = false },
            onPlay = onPlayCached,
        )
        return
    }

    val user = state.user
    val hasSession = user != null || state.hasCookie
    val avatar = rememberRemoteImage(state.userFace)
    var showClearCacheConfirm by remember { mutableStateOf(false) }

    val context = LocalContext.current

    var showCacheLimitDialog by remember { mutableStateOf(false) }
    var showCacheModeDialog by remember { mutableStateOf(false) }
    var cacheLimitMb by remember { mutableStateOf(CachePrefs.limitMb(context)) }
    var cacheMode by remember { mutableStateOf(CachePrefs.mode(context)) }

    // Entering this screen is the natural point to re-stat the cache folder.
    LaunchedEffect(Unit) {
        CoreHolder.core.refreshCacheStats()
    }

    Box(modifier = Modifier.fillMaxSize()) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
        item {
            ScreenHeader(
                kicker = "ACCOUNT",
                title = "我的",
                subtitle = "账号、同步与关于",
            )
        }

        item {
            GlassCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        modifier = Modifier
                            .size(64.dp)
                            .border(
                                width = if (state.vip) 1.5.dp else 1.dp,
                                color = if (state.vip) {
                                    BvoxColors.BrandPrimary
                                } else {
                                    BvoxColors.BrandPrimary.copy(alpha = 0.45f)
                                },
                                shape = CircleShape,
                            )
                            .padding(3.dp)
                            .clip(CircleShape)
                            .background(BvoxColors.Accent),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (avatar != null) {
                            Image(
                                bitmap = avatar,
                                contentDescription = "头像",
                                modifier = Modifier.fillMaxSize(),
                                contentScale = ContentScale.Crop,
                            )
                        } else if (user == null) {
                            Icon(
                                imageVector = Icons.Rounded.Person,
                                contentDescription = null,
                                tint = Color.White,
                                modifier = Modifier.size(28.dp),
                            )
                        } else {
                            Text(
                                text = user.take(1).uppercase(),
                                style = MaterialTheme.typography.titleLarge,
                                color = Color.White,
                                fontWeight = FontWeight.Bold,
                            )
                        }
                    }
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .padding(horizontal = 14.dp),
                    ) {
                        Text(
                            text = when {
                                user != null -> "@$user"
                                state.hasCookie -> "离线"
                                else -> "未登录"
                            },
                            style = MaterialTheme.typography.titleMedium,
                            color = BvoxColors.TextPrimary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            if (state.vip) {
                                VipBadge()
                                Spacer(modifier = Modifier.width(6.dp))
                            }
                            Text(
                                text = when {
                                    user != null -> "已登录 B 站账号"
                                    state.hasCookie -> "离线，已保存登录状态"
                                    else -> "登录后可同步收藏夹、查看歌词"
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = BvoxColors.TextTertiary,
                            )
                        }
                    }
                    if (hasSession) {
                        TextButton(onClick = onLogout) {
                            Icon(
                                imageVector = Icons.AutoMirrored.Rounded.Logout,
                                contentDescription = null,
                                tint = BvoxColors.TextTertiary,
                                modifier = Modifier.size(18.dp),
                            )
                            Spacer(modifier = Modifier.width(4.dp))
                            Text("退出", color = BvoxColors.TextTertiary)
                        }
                    } else {
                        PillButton(text = "登录", onClick = onLogin)
                    }
                }
            }
        }

        item {
            val overlayOn = LyricDisplayPrefs.isOverlayEnabled(context)
            val notificationOn = LyricDisplayPrefs.isNotificationLyrics(context)
            GlassCard(onClick = { showLyricSettings = true }) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        modifier = Modifier
                            .size(44.dp)
                            .clip(CircleShape)
                            .background(BvoxColors.BrandPrimarySoft),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Lyrics,
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
                            text = "歌词显示",
                            style = MaterialTheme.typography.titleSmall,
                            color = BvoxColors.TextPrimary,
                        )
                        Text(
                            text = "桌面歌词${if (overlayOn) "开" else "关"} · " +
                                "通知栏歌词${if (notificationOn) "开" else "关"}",
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                    }
                    Text(
                        text = "设置",
                        style = MaterialTheme.typography.labelLarge,
                        color = BvoxColors.BrandPrimary,
                    )
                }
            }
        }

        item {
            GlassCard(onClick = { showCachedTracks = true }) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        modifier = Modifier
                            .size(44.dp)
                            .clip(CircleShape)
                            .background(BvoxColors.BrandPrimarySoft),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.MusicNote,
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
                            text = "已缓存歌曲",
                            style = MaterialTheme.typography.titleSmall,
                            color = BvoxColors.TextPrimary,
                        )
                        Text(
                            text = if (state.cachedTracks.isEmpty()) {
                                "断网也能播放，播放过的歌会自动缓存"
                            } else {
                                "${state.cachedTracks.size} 首 · 断网也能播放"
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                    }
                    Text(
                        text = "查看 ›",
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.BrandPrimary,
                    )
                }
            }
        }

        item {
            CacheCard(
                usage = state.cacheUsage,
                mode = cacheMode,
                limitMb = cacheLimitMb,
                onAdjustMode = { showCacheModeDialog = true },
                onAdjustLimit = { showCacheLimitDialog = true },
                onClear = { showClearCacheConfirm = true },
            )
        }

        item {
            // Read the installed versionName so the card can never drift from build.gradle.
            val context = LocalContext.current
            val appVersion = remember {
                runCatching {
                    context.packageManager.getPackageInfo(context.packageName, 0).versionName
                }.getOrNull().orEmpty()
            }
            GlassCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        modifier = Modifier
                            .size(44.dp)
                            .clip(CircleShape)
                            .background(BvoxColors.BrandPrimarySoft),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.MusicNote,
                            contentDescription = null,
                            tint = BvoxColors.BrandPrimary,
                            modifier = Modifier.size(22.dp),
                        )
                    }
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .padding(start = 14.dp),
                    ) {
                        Text(
                            text = if (appVersion.isBlank()) "Bvox" else "Bvox $appVersion",
                            style = MaterialTheme.typography.titleSmall,
                            color = BvoxColors.TextPrimary,
                        )
                        Text(
                            text = "Rust 核心 · Media3 播放引擎",
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                    }
                }
            }
        }

        item {
            StatusLine(status = state.status, loading = state.loading, error = state.error)
        }
        }

        if (showClearCacheConfirm) {
            AlertDialog(
                onDismissRequest = { showClearCacheConfirm = false },
                title = { Text("清理缓存") },
                text = {
                    Text("将删除已缓存的音频、封面、歌词和视频信息；正在播放的缓存文件会保留。")
                },
                confirmButton = {
                    TextButton(onClick = {
                        CoreHolder.core.clearCache()
                        showClearCacheConfirm = false
                    }) { Text("清理") }
                },
                dismissButton = {
                    TextButton(onClick = { showClearCacheConfirm = false }) { Text("取消") }
                },
            )
        }
        if (showCacheModeDialog) {
            CacheModeDialog(
                current = cacheMode,
                onDismiss = { showCacheModeDialog = false },
                onSelect = { mode ->
                    // Persist first so a restart keeps the choice even if the
                    // live update fails.
                    CachePrefs.setMode(context, mode)
                    cacheMode = mode
                    CoreHolder.core.setCacheMode(mode)
                    showCacheModeDialog = false
                },
            )
        }
        if (showCacheLimitDialog) {
            CacheLimitDialog(
                currentMb = cacheLimitMb,
                onDismiss = { showCacheLimitDialog = false },
                onSelect = { limitMb ->
                    // Persist first so a restart keeps the choice even if the
                    // live update fails.
                    CachePrefs.setLimitMb(context, limitMb)
                    cacheLimitMb = limitMb
                    CoreHolder.core.setCacheLimitMb(limitMb.toUInt())
                    showCacheLimitDialog = false
                },
            )
        }
    }
}

/** Sizes are held in MiB so they can animate smoothly between snapshots. */
private const val BYTES_PER_MB = 1024.0 * 1024.0

/** One colored segment of the cache bar; the legend uses the same values. */
private data class CacheSlice(val label: String, val mb: Float, val color: Color)

/** Animated per-category size in MiB; snapshots glide instead of snapping. */
@Composable
private fun animatedCacheMb(bytes: ULong?): Float =
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
private fun CacheCard(
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
private fun CacheChip(
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
@Composable
private fun CacheUsageBar(
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
private fun CacheLegend(slices: List<CacheSlice>, modifier: Modifier = Modifier) {
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
private fun formatBytes(bytes: Double): String = when {
    bytes >= 1024.0 * 1024.0 * 1024.0 ->
        String.format(Locale.US, "%.1f GB", bytes / (1024.0 * 1024.0 * 1024.0))
    bytes >= 1024.0 * 1024.0 ->
        String.format(Locale.US, "%.1f MB", bytes / (1024.0 * 1024.0))
    bytes >= 1024.0 ->
        String.format(Locale.US, "%.1f KB", bytes / 1024.0)
    else -> "${bytes.roundToInt()} B"
}

/** Small 大会员 tag; Bilibili's membership color is pink, not gold. */
@Composable
private fun VipBadge() {
    val shape = RoundedCornerShape(BvoxRadius.Pill)
    Text(
        text = "大会员",
        modifier = Modifier
            .clip(shape)
            .background(BvoxColors.BrandPrimarySoft)
            .border(1.dp, BvoxColors.BrandPrimary.copy(alpha = 0.4f), shape)
            .padding(horizontal = 8.dp, vertical = 2.dp),
        style = MaterialTheme.typography.labelSmall,
        color = BvoxColors.BrandPrimary,
        fontWeight = FontWeight.SemiBold,
    )
}
