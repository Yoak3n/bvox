package dev.yoa.bilimusic

import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Foreground service owning the ExoPlayer instance and the media session.
 * The MediaStyle notification (play/pause/seek from the notification shade
 * and lock screen) is provided by Media3's default notification provider.
 *
 * The Rust core never touches this service directly: `ExoPlayerBridge`
 * forwards commands through a `MediaController`, and the state reporter in
 * `MainActivity` pushes ExoPlayer state back into the core.
 */
class PlaybackService : MediaSessionService() {

    private var mediaSession: MediaSession? = null

    override fun onCreate() {
        super.onCreate()

        val httpFactory = DefaultHttpDataSource.Factory()
            .setConnectTimeoutMs(10_000)
            .setReadTimeoutMs(10_000)
            // Bilibili CDNs reject headerless requests; the values come from
            // the Rust core so they stay in sync with api.rs.
            .setUserAgent(dev.yoa.bilimusic.core.playerUserAgent())
            .setDefaultRequestProperties(
                mapOf("Referer" to dev.yoa.bilimusic.core.playerReferer())
            )

        val player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(httpFactory))
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

        mediaSession = MediaSession.Builder(this, player).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession

    override fun onDestroy() {
        mediaSession?.run {
            player.release()
            release()
        }
        mediaSession = null
        super.onDestroy()
    }
}
