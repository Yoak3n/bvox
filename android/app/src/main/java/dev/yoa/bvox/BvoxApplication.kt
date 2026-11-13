package dev.yoa.bvox

import android.app.Application

/**
 * Initializes the core before any component runs. Background playback and the
 * lyric timeline live in `PlaybackService`, so the core must not depend on
 * `MainActivity` having been opened first.
 */
class BvoxApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        CoreHolder.init(this)
        CoreHolder.connectController(this)
    }
}
