package dev.yoa.bvox.lyrics

import android.content.Context
import android.content.SharedPreferences

/**
 * Display preferences for the floating lyric window and the media
 * notification. The overlay keys keep the original `lyric_overlay` file so an
 * already-enabled overlay survives the upgrade.
 */
internal object LyricDisplayPrefs {
    private const val PREFS = "lyric_overlay"
    const val KEY_ENABLED = "enabled"
    private const val KEY_Y = "y"
    const val KEY_LOCKED = "locked"

    const val KEY_FONT_SIZE = "font_size"
    const val KEY_TEXT_COLOR = "text_color"
    const val KEY_DUAL_LINE = "dual_line"
    const val KEY_BACKGROUND = "background"
    const val KEY_OUTLINE = "outline"
    const val KEY_NOTIFICATION_LYRICS = "notification_lyrics"

    private const val DEFAULT_FONT_SIZE = 18
    private const val MIN_FONT_SIZE = 12
    private const val MAX_FONT_SIZE = 32

    private fun prefs(context: Context): SharedPreferences =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    // -- floating window -----------------------------------------------------

    fun isOverlayEnabled(context: Context): Boolean =
        prefs(context).getBoolean(KEY_ENABLED, false)

    fun setOverlayEnabled(context: Context, enabled: Boolean) {
        prefs(context).edit().putBoolean(KEY_ENABLED, enabled).apply()
    }

    /** Last vertical position, or null to use the default below the status bar. */
    fun y(context: Context): Int? {
        val prefs = prefs(context)
        return if (prefs.contains(KEY_Y)) prefs.getInt(KEY_Y, 0) else null
    }

    fun setY(context: Context, y: Int) {
        prefs(context).edit().putInt(KEY_Y, y).apply()
    }

    fun isLocked(context: Context): Boolean =
        prefs(context).getBoolean(KEY_LOCKED, false)

    fun setLocked(context: Context, locked: Boolean) {
        prefs(context).edit().putBoolean(KEY_LOCKED, locked).apply()
    }

    fun fontSize(context: Context): Int =
        prefs(context).getInt(KEY_FONT_SIZE, DEFAULT_FONT_SIZE)
            .coerceIn(MIN_FONT_SIZE, MAX_FONT_SIZE)

    fun setFontSize(context: Context, size: Int) {
        prefs(context).edit()
            .putInt(KEY_FONT_SIZE, size.coerceIn(MIN_FONT_SIZE, MAX_FONT_SIZE))
            .apply()
    }

    /** ARGB text color for the overlay; defaults to white. */
    fun textColor(context: Context): Int =
        prefs(context).getInt(KEY_TEXT_COLOR, 0xFFFFFFFF.toInt())

    fun setTextColor(context: Context, color: Int) {
        prefs(context).edit().putInt(KEY_TEXT_COLOR, color).apply()
    }

    /** Current line + translation / next line in the overlay. */
    fun isDualLine(context: Context): Boolean =
        prefs(context).getBoolean(KEY_DUAL_LINE, true)

    fun setDualLine(context: Context, enabled: Boolean) {
        prefs(context).edit().putBoolean(KEY_DUAL_LINE, enabled).apply()
    }

    /** Rounded translucent backdrop behind the overlay text; off by default. */
    fun isBackground(context: Context): Boolean =
        prefs(context).getBoolean(KEY_BACKGROUND, false)

    fun setBackground(context: Context, enabled: Boolean) {
        prefs(context).edit().putBoolean(KEY_BACKGROUND, enabled).apply()
    }

    /** Thin text outline for readability without a background box. */
    fun isOutline(context: Context): Boolean =
        prefs(context).getBoolean(KEY_OUTLINE, true)

    fun setOutline(context: Context, enabled: Boolean) {
        prefs(context).edit().putBoolean(KEY_OUTLINE, enabled).apply()
    }

    // -- media notification --------------------------------------------------

    fun isNotificationLyrics(context: Context): Boolean =
        prefs(context).getBoolean(KEY_NOTIFICATION_LYRICS, true)

    fun setNotificationLyrics(context: Context, enabled: Boolean) {
        prefs(context).edit().putBoolean(KEY_NOTIFICATION_LYRICS, enabled).apply()
    }

    // -- change notifications ------------------------------------------------

    fun register(
        context: Context,
        listener: SharedPreferences.OnSharedPreferenceChangeListener,
    ) {
        prefs(context).registerOnSharedPreferenceChangeListener(listener)
    }

    fun unregister(
        context: Context,
        listener: SharedPreferences.OnSharedPreferenceChangeListener,
    ) {
        prefs(context).unregisterOnSharedPreferenceChangeListener(listener)
    }
}
