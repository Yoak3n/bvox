package dev.yoa.bvox.playback

import android.app.PendingIntent
import android.content.Intent
import android.content.SharedPreferences
import android.os.Bundle
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import dev.yoa.bvox.core.LyricLine
import dev.yoa.bvox.core.PlayerState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import dev.yoa.bvox.core.CoreHolder
import dev.yoa.bvox.lyrics.LyricDisplayPrefs
import dev.yoa.bvox.lyrics.LyricNotificationProvider
import dev.yoa.bvox.lyrics.LyricOverlayController
import dev.yoa.bvox.lyrics.LyricUiLine
import dev.yoa.bvox.R
import dev.yoa.bvox.MainActivity



/**
 * Foreground service owning the ExoPlayer instance and the media session.
 * The MediaStyle notification (title/artist/artwork, seek, previous/next,
 * lock screen controls) is provided by Media3's notification provider;
 * `LyricNotificationProvider` adds the current lyric line and, on Android 16+,
 * the Live Update capsule.
 *
 * The service also owns the position reporter and the lyric timeline, so they
 * keep running after `MainActivity` is gone. `ExoPlayerBridge` forwards
 * commands through a `MediaController`; the core receives player state from
 * [startStateReporter] exactly like the TUI's mpv poller.
 */
class PlaybackService : MediaSessionService() {

    private var mediaSession: MediaSession? = null
    private lateinit var player: ExoPlayer

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private var lyricLines: List<LyricLine> = emptyList()
    /** `bvid#cid#page` of the track the lyric list belongs to. */
    private var lyricTrackKey = ""
    private var lyricCount = -1
    private var lyricJob: Job? = null
    private var lastLyric: LyricUiLine? = null
    private var originalArtist: CharSequence? = null
    private var overlayController: LyricOverlayController? = null

    private val prefsListener = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        if (key == LyricDisplayPrefs.KEY_NOTIFICATION_LYRICS) {
            // Apply the new toggle to the line already on screen instead of
            // waiting for the next one.
            publishLyricLine(lastLyric, force = true)
        }
    }

    override fun onCreate() {
        // Must be called before super.onCreate(): the service creates the
        // notification channel during its own startup.
        setMediaNotificationProvider(LyricNotificationProvider(this))
        super.onCreate()

        val httpFactory = DefaultHttpDataSource.Factory()
            .setConnectTimeoutMs(10_000)
            .setReadTimeoutMs(10_000)
            // Bilibili CDNs reject headerless requests; the values come from
            // the Rust core so they stay in sync with api.rs.
            .setUserAgent(dev.yoa.bvox.core.playerUserAgent())
            .setDefaultRequestProperties(
                mapOf("Referer" to dev.yoa.bvox.core.playerReferer())
            )
        // Cached tracks are local files; DefaultDataSource routes file:// to
        // FileDataSource and everything else to the http factory above.
        val dataSourceFactory = DefaultDataSource.Factory(this, httpFactory)

        player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build(),
                /* handleAudioFocus = */ true,
            )
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        player.addListener(lyricListener)

        // Cover art lives on the same CDN as the audio, so reuse the factory
        // (and its UA/Referer) instead of Media3's header-less default.
        val bitmapLoader = DataSourceBitmapLoader.Builder(this)
            .setDataSourceFactory(dataSourceFactory)
            .build()

        val openApp = PendingIntent.getActivity(
            this,
            0,
            // Bring an existing activity forward instead of stacking another.
            Intent(this, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        mediaSession = MediaSession.Builder(this, player)
            .setBitmapLoader(bitmapLoader)
            .setSessionActivity(openApp)
            .setCallback(QueueCallback())
            .build()
            .apply { setMediaButtonPreferences(skipButtons()) }

        originalArtist = player.currentMediaItem?.mediaMetadata?.albumArtist
            ?: player.currentMediaItem?.mediaMetadata?.artist
        startStateReporter()
        startLyricUpdates()
        LyricDisplayPrefs.register(this, prefsListener)
        // The overlay rides on this service's media-playback foreground
        // notification, so it needs no notification of its own.
        overlayController = LyricOverlayController(this, scope).also { it.syncVisibility() }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession

    override fun onDestroy() {
        overlayController?.release()
        overlayController = null
        LyricDisplayPrefs.unregister(this, prefsListener)
        scope.cancel()
        if (::player.isInitialized) {
            player.removeListener(lyricListener)
        }
        mediaSession?.release()
        mediaSession = null
        if (::player.isInitialized) {
            player.release()
        }
        super.onDestroy()
    }

    // -- player -> core ------------------------------------------------------

    /**
     * Media3 callbacks fire only on state changes; poll the player twice a
     * second (mirroring the desktop mpv poller) and push snapshots into the
     * core, which drives progress rendering and EOF auto-advance. Living in
     * the service keeps auto-advance working while the UI is closed.
     */
    private fun startStateReporter() {
        scope.launch {
            while (isActive) {
                if (CoreHolder.isReady) {
                    val duration = player.duration
                    CoreHolder.core.reportPlayerState(
                        PlayerState(
                            position = player.currentPosition / 1000.0,
                            duration = if (duration != C.TIME_UNSET && duration > 0) {
                                duration / 1000.0
                            } else {
                                0.0
                            },
                            // A failed load leaves the player idle at 0:00; report it
                            // as paused so the UI does not pretend to be playing.
                            paused = player.playerError != null || !player.playWhenReady,
                            volume = player.volume * 100.0,
                            eof = player.playbackState == Player.STATE_ENDED,
                            idle = player.playbackState == Player.STATE_IDLE,
                        )
                    )
                }
                delay(500)
            }
        }
    }

    // -- lyrics --------------------------------------------------------------

    /** Tracks the core's lyric list for the current track. */
    private fun startLyricUpdates() {
        scope.launch {
            CoreHolder.model.snapshot.collect { snapshot ->
                if (snapshot == null) return@collect
                // `coverKey` is the same `bvid#cid#page` the lyrics are keyed
                // by; the size changes when the async fetch lands.
                val changed = snapshot.coverKey != lyricTrackKey ||
                    snapshot.lyrics.size != lyricCount
                lyricTrackKey = snapshot.coverKey
                lyricLines = snapshot.lyrics
                lyricCount = snapshot.lyrics.size
                if (changed) restartLyricLoop()
            }
        }
    }

    /**
     * Publishes a line whenever playback crosses its timestamp. One coroutine
     * per track, waking at the next line boundary — no 60 fps polling.
     */
    private fun restartLyricLoop() {
        lyricJob?.cancel()
        lyricJob = scope.launch {
            while (isActive) {
                val time = player.currentPosition / 1000.0 + LYRIC_OFFSET
                val index = lyricLines.indexOfLast { it.from <= time }
                // Bilingual subtitles put the translation on a second line;
                // otherwise the overlay's second row is the next lyric line.
                val parts = lyricLines.getOrNull(index)?.text
                    ?.split('\n')
                    ?.map { it.trim() }
                    ?.filter { it.isNotEmpty() }
                    .orEmpty()
                val primary = parts.firstOrNull()
                val line = primary?.let {
                    LyricUiLine(
                        primary = it,
                        translation = parts.drop(1)
                            .joinToString(" ")
                            .takeIf { translation -> translation.isNotEmpty() },
                        next = nextLine(index + 1),
                    )
                }
                publishLyricLine(line)
                if (!player.isPlaying) break
                val next = lyricLines.getOrNull(index + 1)?.from ?: break
                val wait = ((next - LYRIC_OFFSET) * 1000.0 - player.currentPosition).toLong()
                delay(wait.coerceIn(20L, 5_000L))
            }
        }
    }

    /** First non-empty line after [from], used for the overlay's second row. */
    private fun nextLine(from: Int): String? {
        for (i in from until lyricLines.size) {
            val text = lyricLines[i].text.substringBefore('\n').trim()
            if (text.isNotEmpty()) return text
        }
        return null
    }

    /**
     * Writes the line into the current MediaItem. The URI is unchanged, so
     * `ProgressiveMediaSource.canUpdateMediaItem` accepts the replacement and
     * playback keeps its position; Media3 then rebuilds the notification and
     * the platform media metadata (lock screen / QS) picks up the new line.
     *
     * When notification lyrics are disabled the overlay still receives the
     * line, but the media metadata goes back to the real artist.
     */
    private fun publishLyricLine(line: LyricUiLine?, force: Boolean = false) {
        if (!force && line == lastLyric) return
        lastLyric = line
        CoreHolder.lyricLine.value = line

        val item = player.currentMediaItem ?: return
        val index = player.currentMediaItemIndex
        if (index == C.INDEX_UNSET) return
        val baseArtist = originalArtist
            ?: item.mediaMetadata.albumArtist
            ?: item.mediaMetadata.artist
        val showInNotification = LyricDisplayPrefs.isNotificationLyrics(this)
        val primary = line?.primary
        val artist = if (showInNotification) primary ?: baseArtist else baseArtist
        val subtitle = if (showInNotification) primary else null
        if (item.mediaMetadata.artist == artist && item.mediaMetadata.subtitle == subtitle) {
            return
        }
        val metadata = item.mediaMetadata.buildUpon()
            .setSubtitle(subtitle)
            .setArtist(artist)
            .build()
        runCatching {
            player.replaceMediaItem(index, item.buildUpon().setMediaMetadata(metadata).build())
        }
    }

    private val lyricListener = object : Player.Listener {
        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            originalArtist = mediaItem?.mediaMetadata?.albumArtist
                ?: mediaItem?.mediaMetadata?.artist
            lastLyric = null
            // Drop the previous track's lines so the first frame of the new
            // track cannot flash the old lyric; the next snapshot supplies
            // the new list (and restarts the loop when its key changes).
            lyricLines = emptyList()
            CoreHolder.lyricLine.value = null
            restartLyricLoop()
        }

        override fun onEvents(player: Player, events: Player.Events) {
            if (events.containsAny(
                    Player.EVENT_POSITION_DISCONTINUITY,
                    Player.EVENT_IS_PLAYING_CHANGED,
                    Player.EVENT_PLAYBACK_STATE_CHANGED,
                )
            ) {
                restartLyricLoop()
            }
        }
    }

    /**
     * ExoPlayer only ever holds the current track, so its own previous/next
     * commands are meaningless. The queue lives in the Rust core; expose
     * previous/next as custom commands and let a notification button click
     * advance that queue instead.
     */
    private inner class QueueCallback : MediaSession.Callback {

        override fun onConnect(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
        ): MediaSession.ConnectionResult {
            if (!controller.isTrusted) {
                return MediaSession.ConnectionResult.accept(
                    MediaSession.ConnectionResult.DEFAULT_UNTRUSTED_SESSION_COMMANDS,
                    MediaSession.ConnectionResult.DEFAULT_UNTRUSTED_PLAYER_COMMANDS,
                )
            }
            val commands = MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS
                .buildUpon()
                .add(SessionCommand(ACTION_PREVIOUS, Bundle.EMPTY))
                .add(SessionCommand(ACTION_NEXT, Bundle.EMPTY))
                .build()
            return MediaSession.ConnectionResult.accept(
                commands,
                MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS,
            )
        }

        override fun onCustomCommand(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
            customCommand: SessionCommand,
            args: Bundle,
        ): ListenableFuture<SessionResult> {
            if (!CoreHolder.isReady) {
                return Futures.immediateFuture(
                    SessionResult(SessionResult.RESULT_ERROR_INVALID_STATE)
                )
            }
            val result = when (customCommand.customAction) {
                ACTION_PREVIOUS -> {
                    CoreHolder.core.previousTrack()
                    SessionResult.RESULT_SUCCESS
                }
                ACTION_NEXT -> {
                    CoreHolder.core.nextTrack()
                    SessionResult.RESULT_SUCCESS
                }
                else -> SessionResult.RESULT_ERROR_NOT_SUPPORTED
            }
            return Futures.immediateFuture(SessionResult(result))
        }
    }

    /** Back/forward slots; Media3 fills the central play/pause button itself. */
    private fun skipButtons(): List<CommandButton> = listOf(
        CommandButton.Builder(CommandButton.ICON_PREVIOUS)
            .setSessionCommand(SessionCommand(ACTION_PREVIOUS, Bundle.EMPTY))
            .setDisplayName(getString(R.string.previous_track))
            .setSlots(CommandButton.SLOT_BACK)
            .build(),
        CommandButton.Builder(CommandButton.ICON_NEXT)
            .setSessionCommand(SessionCommand(ACTION_NEXT, Bundle.EMPTY))
            .setDisplayName(getString(R.string.next_track))
            .setSlots(CommandButton.SLOT_FORWARD)
            .build(),
    )

    private companion object {
        const val ACTION_PREVIOUS = "dev.yoa.bvox.PREVIOUS"
        const val ACTION_NEXT = "dev.yoa.bvox.NEXT"

        /** Same lead-in as the core's `App::lyric_cursor`. */
        const val LYRIC_OFFSET = 0.2
    }
}
