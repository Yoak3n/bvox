package dev.yoa.bvox.ui.screens.nowplaying

import dev.yoa.bvox.core.AudioQuality
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.dialogs.audioQualityLabel

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
