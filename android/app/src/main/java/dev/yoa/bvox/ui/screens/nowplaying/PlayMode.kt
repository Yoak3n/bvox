package dev.yoa.bvox.ui.screens.nowplaying

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistPlay
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.ui.graphics.vector.ImageVector

import dev.yoa.bvox.core.PlayMode

internal fun PlayMode.next(): PlayMode = when (this) {
    PlayMode.SEQUENTIAL -> PlayMode.REPEAT_ALL
    PlayMode.REPEAT_ALL -> PlayMode.REPEAT_ONE
    PlayMode.REPEAT_ONE -> PlayMode.SHUFFLE
    PlayMode.SHUFFLE -> PlayMode.SEQUENTIAL
}

internal fun playModeLabel(mode: PlayMode): String = when (mode) {
    PlayMode.SEQUENTIAL -> "顺序播放"
    PlayMode.REPEAT_ALL -> "列表循环"
    PlayMode.REPEAT_ONE -> "单曲循环"
    PlayMode.SHUFFLE -> "随机播放"
}

internal fun playModeIcon(mode: PlayMode): ImageVector = when (mode) {
    PlayMode.SEQUENTIAL -> Icons.AutoMirrored.Rounded.PlaylistPlay
    PlayMode.REPEAT_ALL -> Icons.Rounded.Repeat
    PlayMode.REPEAT_ONE -> Icons.Rounded.RepeatOne
    PlayMode.SHUFFLE -> Icons.Rounded.Shuffle
}
