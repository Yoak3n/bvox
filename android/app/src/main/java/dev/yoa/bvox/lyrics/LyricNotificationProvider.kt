package dev.yoa.bvox.lyrics

import android.app.Notification
import android.content.Context
import android.os.Build
import android.os.Bundle
import androidx.media3.session.CommandButton
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaNotification
import androidx.media3.session.MediaSession
import com.google.common.collect.ImmutableList
import dev.yoa.bvox.R

/**
 * Media notification provider with the current lyric line in the body and,
 * on Android 16+, a promoted Live Update capsule in the status bar.
 *
 * The line arrives via `MediaMetadata.artist` (the lyric timeline swaps it in
 * while keeping the real artist in `albumArtist`), so Media3's default
 * notification already renders it. The default provider's
 * `createNotification` is final, hence a delegating wrapper for the capsule.
 *
 * 16.0 (e.g. ColorOS 16) promotes with `android.colorized=true`; 16.1+ needs
 * `android.requestPromotedOngoing=true` and `colorized=false`.
 */
internal class LyricNotificationProvider(context: Context) : MediaNotification.Provider {

    private val appContext = context.applicationContext

    private val delegate = DefaultMediaNotificationProvider(
        context,
        { _ -> DefaultMediaNotificationProvider.DEFAULT_NOTIFICATION_ID },
        CHANNEL_ID,
        R.string.playback_channel_name,
    ).apply {
        setSmallIcon(R.drawable.ic_notification)
    }

    override fun createNotification(
        mediaSession: MediaSession,
        mediaButtonPreferences: ImmutableList<CommandButton>,
        actionFactory: MediaNotification.ActionFactory,
        onNotificationChangedCallback: MediaNotification.Provider.Callback,
    ): MediaNotification {
        val built = delegate.createNotification(
            mediaSession,
            mediaButtonPreferences,
            actionFactory,
            onNotificationChangedCallback,
        )
        val lyric = mediaSession.player.mediaMetadata.subtitle
            ?.toString()
            ?.replace('\n', ' ')
            ?.trim()
            ?.takeIf { it.isNotEmpty() }
            ?: return built

        return try {
            MediaNotification(
                built.notificationId,
                promote(appContext, built.notification, lyric),
            )
        } catch (_: Throwable) {
            // A ROM that rejects the promotion extras must keep the normal
            // media notification; lyrics in the shade still work.
            built
        }
    }

    override fun handleCustomCommand(
        session: MediaSession,
        action: String,
        extras: Bundle,
    ): Boolean = delegate.handleCustomCommand(session, action, extras)

    override fun getNotificationChannelInfo(): MediaNotification.Provider.NotificationChannelInfo =
        delegate.notificationChannelInfo

    private fun promote(context: Context, notification: Notification, lyric: String): Notification {
        if (Build.VERSION.SDK_INT < 36) {
            return notification
        }
        val builder = Notification.Builder.recoverBuilder(context, notification)
        // The capsule holds only a handful of characters; the full line stays
        // in the notification body and on the lock screen.
        builder.setShortCriticalText(lyric.take(CAPSULE_CHARS))
        builder.addExtras(Bundle().apply {
            if (HAS_PROMOTED_SETTER) {
                putBoolean("android.requestPromotedOngoing", true)
                putBoolean("android.colorized", false)
            } else {
                putBoolean("android.colorized", true)
            }
        })
        return builder.build()
    }

    private companion object {
        const val CHANNEL_ID = "bvox_playback"
        const val CAPSULE_CHARS = 7

        /** Android 16 QPR1+ replaced the colorized model with an explicit request bit. */
        val HAS_PROMOTED_SETTER: Boolean by lazy {
            try {
                Notification.Builder::class.java.getMethod(
                    "setRequestPromotedOngoing",
                    Boolean::class.javaPrimitiveType,
                )
                true
            } catch (_: Throwable) {
                false
            }
        }
    }
}
