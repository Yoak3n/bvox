package dev.yoa.bvox.ui.screens.me

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.lyrics.LyricDisplayPrefs
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.theme.BvoxColors

/** Entry card for the lyric display settings. */
@Composable
internal fun LyricDisplayEntry(onClick: () -> Unit) {
    val context = LocalContext.current
    val overlayOn = LyricDisplayPrefs.isOverlayEnabled(context)
    val notificationOn = LyricDisplayPrefs.isNotificationLyrics(context)
    GlassCard(onClick = onClick) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            EntryIcon(Icons.Rounded.Lyrics)
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

/** Entry card for tracks that are already cached on disk. */
@Composable
internal fun CachedTracksEntry(count: Int, onClick: () -> Unit) {
    GlassCard(onClick = onClick) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            EntryIcon(Icons.Rounded.MusicNote)
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
                    text = if (count == 0) {
                        "断网也能播放，播放过的歌会自动缓存"
                    } else {
                        "$count 首 · 断网也能播放"
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

/** About card; reads the installed versionName so it cannot drift from Gradle. */
@Composable
internal fun AboutCard() {
    val context = LocalContext.current
    val appVersion = remember {
        runCatching {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        }.getOrNull().orEmpty()
    }
    GlassCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            EntryIcon(Icons.Rounded.MusicNote)
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

/** Round tinted icon shared by the entry cards. */
@Composable
private fun EntryIcon(icon: androidx.compose.ui.graphics.vector.ImageVector) {
    Box(
        modifier = Modifier
            .size(44.dp)
            .clip(CircleShape)
            .background(BvoxColors.BrandPrimarySoft),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = BvoxColors.BrandPrimary,
            modifier = Modifier.size(22.dp),
        )
    }
}
