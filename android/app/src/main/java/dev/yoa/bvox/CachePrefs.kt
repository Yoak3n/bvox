package dev.yoa.bvox

import android.content.Context
import dev.yoa.bvox.core.CacheMode

/**
 * User-chosen cache strategy and the cap that applies to audio downloads.
 *
 * The core takes both once at construction, so they also live here to survive
 * restarts; changing them at runtime goes through `Core::setCacheMode` and
 * `Core::setCacheLimitMb`. The cap only matters in auto mode.
 */
object CachePrefs {
    private const val PREFS = "cache_settings"
    private const val KEY_LIMIT_MB = "limit_mb"
    private const val KEY_MODE = "mode"

    const val DEFAULT_LIMIT_MB = 1024

    /** Caps offered in the settings dialog, in MiB (auto mode only). */
    val LIMIT_OPTIONS_MB = listOf(512, 1024, 2048, 4096, 8192)

    /** Strategies in the same order the TUI's one-key switch cycles. */
    val MODE_OPTIONS = listOf(CacheMode.AUTO, CacheMode.SMALL_FILES, CacheMode.OFF)

    fun mode(context: Context): CacheMode {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        return when (prefs.getString(KEY_MODE, null)) {
            "auto" -> CacheMode.AUTO
            "small_files" -> CacheMode.SMALL_FILES
            "off" -> CacheMode.OFF
            // Migration: the old cap picker stored 0 for "缓存关闭".
            null -> if (prefs.getInt(KEY_LIMIT_MB, DEFAULT_LIMIT_MB) <= 0) {
                CacheMode.OFF
            } else {
                CacheMode.AUTO
            }
            else -> CacheMode.AUTO
        }
    }

    fun setMode(context: Context, mode: CacheMode) {
        val value = when (mode) {
            CacheMode.AUTO -> "auto"
            CacheMode.SMALL_FILES -> "small_files"
            CacheMode.OFF -> "off"
        }
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_MODE, value)
            .apply()
    }

    fun limitMb(context: Context): Int {
        val value = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getInt(KEY_LIMIT_MB, DEFAULT_LIMIT_MB)
        // A legacy "0" was the old way to disable caching; the strategy now
        // owns that decision, so keep the cap usable.
        return if (value <= 0) DEFAULT_LIMIT_MB else value
    }

    fun setLimitMb(context: Context, limitMb: Int) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putInt(KEY_LIMIT_MB, limitMb.coerceAtLeast(0))
            .apply()
    }

    /** Short strategy label for chips and summaries. */
    fun modeLabel(mode: CacheMode): String = when (mode) {
        CacheMode.AUTO -> "自动缓存"
        CacheMode.SMALL_FILES -> "仅小文件"
        CacheMode.OFF -> "关闭"
    }

    /** One-line explanation shown in the strategy dialog. */
    fun modeDescription(mode: CacheMode): String = when (mode) {
        CacheMode.AUTO -> "播放时后台保存音频，下次播放直接用本地文件"
        CacheMode.SMALL_FILES -> "音频始终在线播放，只缓存封面、歌词和视频信息"
        CacheMode.OFF -> "不读也不写缓存，已有文件保留到手动清理"
    }

    /** Short label for the current cap ("2 GB", "512 MB"). */
    fun label(limitMb: Int): String = when {
        limitMb <= 0 -> "关闭"
        limitMb % 1024 == 0 -> "${limitMb / 1024} GB"
        else -> "$limitMb MB"
    }
}
