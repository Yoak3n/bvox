package dev.yoa.bvox.ui.screens

import android.content.Intent
import android.content.SharedPreferences
import android.net.Uri
import android.provider.Settings
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Subtitles
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.LyricDisplayPrefs
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.theme.BvoxColors
import kotlin.math.roundToInt

/** Text color presets for the floating lyric window. */
private val LyricColorChoices = listOf(
    Color.White,
    Color(0xFFFB7299),
    Color(0xFF8B7CFF),
    Color(0xFF4DD0E1),
    Color(0xFFFFD54F),
    Color(0xFF9AE66E),
)

/**
 * Second-level lyric display settings: the floating window and the media
 * notification. Kept out of the "我的" list so the account/cache cards stay
 * scannable.
 */
@Composable
fun LyricSettingsScreen(onBack: () -> Unit) {
    val context = LocalContext.current

    var overlayEnabled by remember {
        mutableStateOf(
            LyricDisplayPrefs.isOverlayEnabled(context) && Settings.canDrawOverlays(context)
        )
    }
    var pendingOverlay by remember { mutableStateOf(false) }
    val overlayPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) {
        if (!pendingOverlay) return@rememberLauncherForActivityResult
        pendingOverlay = false
        if (Settings.canDrawOverlays(context)) {
            LyricDisplayPrefs.setOverlayEnabled(context, true)
            overlayEnabled = true
        } else {
            Toast.makeText(
                context,
                "需要悬浮窗权限才能显示桌面歌词",
                Toast.LENGTH_SHORT,
            ).show()
        }
    }

    var fontSize by remember { mutableStateOf(LyricDisplayPrefs.fontSize(context).toFloat()) }
    var textColor by remember { mutableStateOf(LyricDisplayPrefs.textColor(context)) }
    var dualLine by remember { mutableStateOf(LyricDisplayPrefs.isDualLine(context)) }
    var overlayLocked by remember { mutableStateOf(LyricDisplayPrefs.isLocked(context)) }
    var lyricBackground by remember { mutableStateOf(LyricDisplayPrefs.isBackground(context)) }
    var outline by remember { mutableStateOf(LyricDisplayPrefs.isOutline(context)) }
    var notificationLyrics by remember {
        mutableStateOf(LyricDisplayPrefs.isNotificationLyrics(context))
    }

    BackHandler(onBack = onBack)

    // The notification's "关闭" action stops the service; keep the switch in
    // sync if the user returns here afterwards.
    DisposableEffect(context) {
        val listener = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            when (key) {
                LyricDisplayPrefs.KEY_ENABLED -> overlayEnabled =
                    LyricDisplayPrefs.isOverlayEnabled(context) &&
                        Settings.canDrawOverlays(context)
                LyricDisplayPrefs.KEY_LOCKED -> overlayLocked =
                    LyricDisplayPrefs.isLocked(context)
            }
        }
        LyricDisplayPrefs.register(context, listener)
        onDispose { LyricDisplayPrefs.unregister(context, listener) }
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onBack) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Rounded.ArrowBack,
                        contentDescription = "返回我的",
                        tint = BvoxColors.TextPrimary,
                    )
                }
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "歌词显示",
                        style = MaterialTheme.typography.titleLarge,
                        color = BvoxColors.TextPrimary,
                    )
                    Text(
                        text = "桌面歌词与通知栏",
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                    )
                }
            }
        }

        item {
            GlassCard {
                Column {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        SectionIcon(Icons.Rounded.Lyrics)
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .padding(horizontal = 14.dp),
                        ) {
                            Text(
                                text = "桌面歌词",
                                style = MaterialTheme.typography.titleSmall,
                                color = BvoxColors.TextPrimary,
                            )
                            Text(
                                text = "悬浮在其他应用上方显示当前歌词",
                                style = MaterialTheme.typography.bodySmall,
                                color = BvoxColors.TextTertiary,
                            )
                        }
                        Switch(
                            checked = overlayEnabled,
                            onCheckedChange = { checked ->
                                if (checked) {
                                    if (Settings.canDrawOverlays(context)) {
                                        LyricDisplayPrefs.setOverlayEnabled(context, true)
                                        overlayEnabled = true
                                    } else {
                                        pendingOverlay = true
                                        overlayPermission.launch(
                                            Intent(
                                                Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                                                Uri.parse("package:${context.packageName}"),
                                            )
                                        )
                                    }
                                } else {
                                    LyricDisplayPrefs.setOverlayEnabled(context, false)
                                    overlayEnabled = false
                                }
                            },
                        )
                    }

                    if (overlayEnabled) {
                        Spacer(modifier = Modifier.height(12.dp))
                        Text(
                            text = "字号 ${fontSize.roundToInt()}sp",
                            style = MaterialTheme.typography.labelMedium,
                            color = BvoxColors.TextSecondary,
                        )
                        Slider(
                            value = fontSize,
                            onValueChange = {
                                fontSize = it
                                LyricDisplayPrefs.setFontSize(context, it.roundToInt())
                            },
                            valueRange = 14f..28f,
                            steps = 13,
                        )
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
                                val selected = argb == textColor
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
                                        .clickable {
                                            textColor = argb
                                            LyricDisplayPrefs.setTextColor(context, argb)
                                        },
                                )
                            }
                        }
                        SettingToggle(
                            title = "双行歌词",
                            subtitle = "优先显示翻译，没有翻译时显示下一行",
                            checked = dualLine,
                            onCheckedChange = {
                                dualLine = it
                                LyricDisplayPrefs.setDualLine(context, it)
                            },
                        )
                        SettingToggle(
                            title = "锁定位置",
                            subtitle = "锁定后拖动会穿透到下层应用",
                            checked = overlayLocked,
                            onCheckedChange = {
                                overlayLocked = it
                                LyricDisplayPrefs.setLocked(context, it)
                            },
                        )
                        SettingToggle(
                            title = "背景框",
                            subtitle = "在文字后显示半透明圆角背景",
                            checked = lyricBackground,
                            onCheckedChange = {
                                lyricBackground = it
                                LyricDisplayPrefs.setBackground(context, it)
                            },
                        )
                        SettingToggle(
                            title = "文字描边",
                            subtitle = "加一圈细描边；开启背景框时自动关闭",
                            checked = outline,
                            onCheckedChange = {
                                outline = it
                                LyricDisplayPrefs.setOutline(context, it)
                            },
                        )
                    }
                }
            }
        }

        item {
            GlassCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SectionIcon(Icons.Rounded.Subtitles)
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .padding(horizontal = 14.dp),
                    ) {
                        Text(
                            text = "通知栏歌词",
                            style = MaterialTheme.typography.titleSmall,
                            color = BvoxColors.TextPrimary,
                        )
                        Text(
                            text = "在通知栏、锁屏和状态栏胶囊显示当前歌词",
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                    }
                    Switch(
                        checked = notificationLyrics,
                        onCheckedChange = {
                            notificationLyrics = it
                            LyricDisplayPrefs.setNotificationLyrics(context, it)
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun SectionIcon(icon: androidx.compose.ui.graphics.vector.ImageVector) {
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

@Composable
private fun SettingToggle(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = BvoxColors.TextPrimary,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = BvoxColors.TextTertiary,
            )
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
