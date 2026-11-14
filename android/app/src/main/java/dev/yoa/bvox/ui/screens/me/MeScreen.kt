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
