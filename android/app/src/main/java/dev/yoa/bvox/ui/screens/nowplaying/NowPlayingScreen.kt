package dev.yoa.bvox.ui.screens.nowplaying

import android.os.Build
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur

import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.core.AppModel

@Composable
internal fun NowPlayingScreen(model: AppModel, modifier: Modifier = Modifier) {
    val snapshot by model.snapshot.collectAsState()
    val cover by model.cover.collectAsState()
    val state = snapshot
    val item = state?.nowPlaying
    val bitmap = cover?.takeIf { it.first == state?.coverKey }?.second

    Box(modifier = modifier.fillMaxSize()) {
        // Modifier.blur needs RenderEffect (API 31); older devices keep the
        // stage gradient instead of showing a sharp cover behind the UI.
        if (bitmap != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            CoverBackdrop(bitmap)
        }
        if (state == null || item == null) {
            EmptyState(
                icon = Icons.Rounded.MusicNote,
                title = "还没有播放歌曲",
                message = "去「发现」或「歌单」挑一首吧",
                modifier = Modifier.fillMaxSize(),
            )
        } else {
            PlayerContent(state, item, bitmap)
        }
    }
}
