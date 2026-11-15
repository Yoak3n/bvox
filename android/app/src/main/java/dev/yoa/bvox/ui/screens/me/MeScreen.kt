package dev.yoa.bvox.ui.screens.me

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.prefs.CachePrefs
import dev.yoa.bvox.ui.components.ScreenHeader
import dev.yoa.bvox.ui.components.StatusLine
import dev.yoa.bvox.ui.dialogs.CacheLimitDialog
import dev.yoa.bvox.ui.dialogs.CacheModeDialog
import dev.yoa.bvox.ui.screens.cached.CachedTracksScreen
import dev.yoa.bvox.ui.screens.lyrics.LyricSettingsScreen

@Composable
internal fun MeScreen(
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
                AccountSection(
                    state = state,
                    onLogin = onLogin,
                    onLogout = onLogout,
                )
            }

            item {
                LyricDisplayEntry(onClick = { showLyricSettings = true })
            }

            item {
                CachedTracksEntry(
                    count = state.cachedTracks.size,
                    onClick = { showCachedTracks = true },
                )
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
                AboutCard()
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
