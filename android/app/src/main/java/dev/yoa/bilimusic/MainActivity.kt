package dev.yoa.bilimusic

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.MaterialTheme
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import dev.yoa.bilimusic.ui.BiliMusicApp
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {

    private val notificationPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val model = CoreHolder.init(applicationContext)
        CoreHolder.connectController(applicationContext)
        startStateReporter()

        requestNotificationPermissionIfNeeded()

        setContent {
            MaterialTheme {
                BiliMusicApp(model)
            }
        }
    }

    /**
     * Media3 fires callbacks only on state changes; poll the controller twice
     * a second (mirroring the desktop mpv poller) and push snapshots into the
     * core, which drives progress rendering and EOF auto-advance.
     *
     * Must stay on the main dispatcher: MediaController methods are
     * main-thread-only.
     */
    private fun startStateReporter() {
        lifecycleScope.launch {
            while (isActive) {
                CoreHolder.controllerFlow.value?.let { controller ->
                    val ended = controller.playbackState == androidx.media3.common.Player.STATE_ENDED
                    val idle = controller.playbackState == androidx.media3.common.Player.STATE_IDLE
                    val duration = controller.duration
                    CoreHolder.core.reportPlayerState(
                        dev.yoa.bilimusic.core.PlayerState(
                            position = controller.currentPosition / 1000.0,
                            duration = if (duration != androidx.media3.common.C.TIME_UNSET && duration > 0) duration / 1000.0 else 0.0,
                            paused = !controller.playWhenReady,
                            volume = controller.volume * 100.0,
                            eof = ended,
                            idle = idle,
                        )
                    )
                }
                delay(500)
            }
        }
    }

    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED
            ) {
                notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }
}
