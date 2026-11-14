package dev.yoa.bvox.ui.screens.nowplaying

import android.os.Build
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.PlaylistPlay
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp


import dev.yoa.bvox.core.AudioQuality
import dev.yoa.bvox.core.PlayMode
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem
import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.InfoChip
import dev.yoa.bvox.ui.components.PlayButton
import dev.yoa.bvox.ui.components.TrackRow
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius
import dev.yoa.bvox.core.AppModel
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.ui.dialogs.PagePickerDialog
import dev.yoa.bvox.ui.dialogs.audioQualityLabel
import dev.yoa.bvox.ui.dialogs.AudioQualityDialog


internal data class TrackChip(val text: String, val action: (() -> Unit)? = null)

internal fun trackChips(
    state: Snapshot,
    onOpenPages: () -> Unit,
    onOpenQuality: () -> Unit,
): List<TrackChip> {
    val chips = mutableListOf<TrackChip>()
    val page = state.page.toInt()
    val pageCount = state.pageCount.toInt()
    if (pageCount > 1) chips += TrackChip("P${page + 1}/$pageCount", onOpenPages)
    if (state.quality.isNotEmpty()) chips += TrackChip(state.quality, onOpenQuality)
    if (state.qualityPref != AudioQuality.AUTO) {
        chips += TrackChip("音质 ${audioQualityLabel(state.qualityPref)}", onOpenQuality)
    }
    // The 分P title already serves as the track heading, so the chips only
    // carry playback facts (stream, cache).
    if (state.cachedNow) chips += TrackChip("已缓存")
    state.cacheProgress?.let { progress ->
        val downloaded = progress.downloaded.toLong()
        val total = progress.total?.toLong()
        val text = if (total != null && total > 0) {
            "缓存中 ${downloaded * 100 / total}%"
        } else {
            "缓存中 ${downloaded / 1_048_576} MB"
        }
        chips += TrackChip(text)
    }
    return chips
}

/** Scrolling lyrics with the active line lifted and tinted. */
