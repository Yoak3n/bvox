package dev.yoa.bvox

import android.content.Context
import android.content.SharedPreferences
import android.app.NotificationManager
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.provider.Settings
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.TextView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch

/**
 * Owns the floating lyric window.
 *
 * It is created by [PlaybackService] and rides on that service's existing
 * media-playback foreground notification, so the overlay needs neither a
 * second foreground service nor its own notification. Lifetime is tied to the
 * playback service: while the app is open the MediaController keeps the
 * service alive, and destroying it removes the window.
 */
class LyricOverlayController(
    private val context: Context,
    scope: CoroutineScope,
) {
    private val windowManager =
        context.getSystemService(Context.WINDOW_SERVICE) as WindowManager

    private var root: LinearLayout? = null
    private var primaryView: OutlinedTextView? = null
    private var secondaryView: OutlinedTextView? = null
    private var params: WindowManager.LayoutParams? = null
    private var currentLine: LyricUiLine? = null
    private var locked = LyricDisplayPrefs.isLocked(context)

    private var dragStartY = 0
    private var dragStartRawY = 0f

    private val lyricsJob: Job = scope.launch {
        CoreHolder.lyricLine.collect { line -> render(line, force = false) }
    }

    private val prefsListener = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        when (key) {
            LyricDisplayPrefs.KEY_ENABLED -> syncVisibility()
            LyricDisplayPrefs.KEY_FONT_SIZE,
            LyricDisplayPrefs.KEY_TEXT_COLOR,
            LyricDisplayPrefs.KEY_DUAL_LINE,
            LyricDisplayPrefs.KEY_BACKGROUND,
            LyricDisplayPrefs.KEY_OUTLINE,
            -> {
                applyStyle()
                render(currentLine, force = true)
            }
            LyricDisplayPrefs.KEY_LOCKED -> {
                locked = LyricDisplayPrefs.isLocked(context)
                applyLock()
            }
        }
    }

    init {
        // The overlay used to be its own foreground service; drop the stale
        // channel so it no longer appears in notification settings.
        if (Build.VERSION.SDK_INT >= 26) {
            context.getSystemService(NotificationManager::class.java)
                .deleteNotificationChannel(LEGACY_CHANNEL_ID)
        }
        LyricDisplayPrefs.register(context, prefsListener)
    }

    /** Shows or hides the window according to the saved preference. */
    fun syncVisibility() {
        if (!LyricDisplayPrefs.isOverlayEnabled(context)) {
            hideOverlay()
            return
        }
        if (!Settings.canDrawOverlays(context)) {
            hideOverlay()
            // Keep the settings switch honest if the permission was revoked.
            LyricDisplayPrefs.setOverlayEnabled(context, false)
            return
        }
        showOverlay()
    }

    fun release() {
        LyricDisplayPrefs.unregister(context, prefsListener)
        lyricsJob.cancel()
        hideOverlay()
    }

    // -- window --------------------------------------------------------------

    private fun showOverlay() {
        if (root != null) return

        val maxTextWidth = (context.resources.displayMetrics.widthPixels * 0.86f).toInt()
        val primary = OutlinedTextView(context).apply {
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            maxLines = 2
            ellipsize = TextUtils.TruncateAt.END
            maxWidth = maxTextWidth
        }
        val secondary = OutlinedTextView(context).apply {
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            maxWidth = maxTextWidth
            setPadding(0, dp(2f), 0, 0)
            visibility = View.GONE
        }
        val container = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(dp(18f), dp(10f), dp(18f), dp(10f))
            visibility = View.GONE
            addView(primary)
            addView(secondary)
        }
        val touch = View.OnTouchListener { _, event -> handleDrag(event) }
        container.setOnTouchListener(touch)
        primary.setOnTouchListener(touch)
        secondary.setOnTouchListener(touch)

        val layout = WindowManager.LayoutParams(
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            overlayWindowType(),
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
            y = (LyricDisplayPrefs.y(context) ?: defaultY()).coerceIn(0, maxY())
        }
        applyLockTo(layout)

        runCatching { windowManager.addView(container, layout) }
            .onSuccess {
                root = container
                primaryView = primary
                secondaryView = secondary
                params = layout
                applyStyle()
                render(CoreHolder.lyricLine.value, force = true)
            }
            .onFailure {
                LyricDisplayPrefs.setOverlayEnabled(context, false)
            }
    }

    private fun hideOverlay() {
        root?.let { view ->
            runCatching { windowManager.removeView(view) }
        }
        root = null
        primaryView = null
        secondaryView = null
        params = null
    }

    private fun applyStyle() {
        val primary = primaryView ?: return
        val secondary = secondaryView ?: return
        val size = LyricDisplayPrefs.fontSize(context).toFloat()
        val color = LyricDisplayPrefs.textColor(context)
        primary.setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        primary.setTextColor(color)
        secondary.setTextSize(TypedValue.COMPLEX_UNIT_SP, size * 0.78f)
        secondary.setTextColor(withAlpha(color, 0.72f))

        // A crisp, tinted outline reads cleanly on both white and dark
        // backgrounds; the old blurred shadow turned into a grey halo on
        // light panels, and a thick black stroke swallowed the glyphs.
        val hasBackground = LyricDisplayPrefs.isBackground(context)
        val showOutline = !hasBackground && LyricDisplayPrefs.isOutline(context)
        val outlineColor = outlineColorFor(color)
        primary.outlineWidth = if (showOutline) dp(0.9f).toFloat() else 0f
        primary.outlineColor = outlineColor
        secondary.outlineWidth = if (showOutline) dp(0.7f).toFloat() else 0f
        secondary.outlineColor = outlineColor

        root?.background = if (hasBackground) {
            GradientDrawable().apply {
                cornerRadius = dp(20f).toFloat()
                setColor(Color.argb(150, 0, 0, 0))
            }
        } else {
            null
        }
    }

    private fun render(line: LyricUiLine?, force: Boolean) {
        currentLine = line
        val container = root ?: return
        val primary = primaryView ?: return
        val secondary = secondaryView ?: return

        val primaryText = line?.primary?.takeIf { it.isNotEmpty() }
        if (primaryText == null) {
            container.animate().cancel()
            container.visibility = View.GONE
            return
        }
        val secondaryText = if (LyricDisplayPrefs.isDualLine(context)) {
            line.translation ?: line.next
        } else {
            null
        }
        val secondaryVisible = secondaryText != null

        val sameText = primary.text.toString() == primaryText &&
            secondary.text.toString() == (secondaryText ?: "") &&
            (secondary.visibility == View.VISIBLE) == secondaryVisible
        if (!force && sameText && container.visibility == View.VISIBLE) return

        container.visibility = View.VISIBLE
        if (force) {
            container.animate().cancel()
            container.alpha = 1f
            primary.text = primaryText
            secondary.text = secondaryText ?: ""
            secondary.visibility = if (secondaryVisible) View.VISIBLE else View.GONE
            return
        }

        container.animate().cancel()
        container.animate()
            .alpha(0f)
            .setDuration(80)
            .withEndAction {
                primary.text = primaryText
                secondary.text = secondaryText ?: ""
                secondary.visibility = if (secondaryVisible) View.VISIBLE else View.GONE
                container.animate().alpha(1f).setDuration(140).start()
            }
            .start()
    }

    // -- interaction ---------------------------------------------------------

    private fun applyLock() {
        val layout = params ?: return
        applyLockTo(layout)
        runCatching { windowManager.updateViewLayout(root, layout) }
    }

    private fun applyLockTo(layout: WindowManager.LayoutParams) {
        layout.flags = if (locked) {
            layout.flags or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        } else {
            layout.flags and WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE.inv()
        }
    }

    private fun handleDrag(event: MotionEvent): Boolean {
        if (locked) return false
        val layout = params ?: return false
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                dragStartY = layout.y
                dragStartRawY = event.rawY
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                layout.y = (dragStartY + (event.rawY - dragStartRawY).toInt())
                    .coerceIn(0, maxY())
                runCatching { windowManager.updateViewLayout(root, layout) }
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                LyricDisplayPrefs.setY(context, layout.y)
                return true
            }
        }
        return false
    }

    // -- geometry ------------------------------------------------------------

    private fun withAlpha(color: Int, factor: Float): Int {
        val alpha = (Color.alpha(color) * factor).toInt().coerceIn(0, 255)
        return (color and 0x00FFFFFF) or (alpha shl 24)
    }

    /** Dark shade of the text color, so the outline never looks like a black blob. */
    private fun outlineColorFor(color: Int): Int {
        val red = (Color.red(color) * 0.45f).toInt()
        val green = (Color.green(color) * 0.45f).toInt()
        val blue = (Color.blue(color) * 0.45f).toInt()
        return Color.argb(230, red, green, blue)
    }

    private fun overlayWindowType(): Int =
        if (Build.VERSION.SDK_INT >= 26) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }

    private fun defaultY(): Int {
        val id = context.resources.getIdentifier("status_bar_height", "dimen", "android")
        val statusBar = if (id > 0) context.resources.getDimensionPixelSize(id) else 0
        return statusBar + dp(24f)
    }

    private fun maxY(): Int =
        (context.resources.displayMetrics.heightPixels - dp(96f)).coerceAtLeast(0)

    private fun dp(value: Float): Int = TypedValue.applyDimension(
        TypedValue.COMPLEX_UNIT_DIP,
        value,
        context.resources.displayMetrics,
    ).toInt()

    private companion object {
        const val LEGACY_CHANNEL_ID = "bili_music_lyric_overlay"
    }
}

/**
 * TextView that draws a crisp outline under the fill. Cheaper and cleaner
 * than a large blurred shadow on light backgrounds.
 */
private class OutlinedTextView @JvmOverloads constructor(
    context: Context,
    attrs: android.util.AttributeSet? = null,
) : TextView(context, attrs) {
    var outlineWidth: Float = 0f
    var outlineColor: Int = Color.BLACK

    override fun onDraw(canvas: Canvas) {
        if (outlineWidth <= 0f) {
            super.onDraw(canvas)
            return
        }
        val paint = paint
        val fillColor = currentTextColor
        val style = paint.style
        val strokeWidth = paint.strokeWidth
        val join = paint.strokeJoin
        val cap = paint.strokeCap
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = outlineWidth
        paint.strokeJoin = Paint.Join.ROUND
        paint.strokeCap = Paint.Cap.ROUND
        paint.color = outlineColor
        super.onDraw(canvas)
        paint.style = style
        paint.strokeWidth = strokeWidth
        paint.strokeJoin = join
        paint.strokeCap = cap
        paint.color = fillColor
        super.onDraw(canvas)
    }
}
