package dev.yoa.bilimusic

import android.content.ComponentName
import android.content.Context
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.media3.common.MediaItem
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import dev.yoa.bilimusic.core.Core
import dev.yoa.bilimusic.core.CoreConfig
import dev.yoa.bilimusic.core.CoreEvent
import dev.yoa.bilimusic.core.CoreObserver
import dev.yoa.bilimusic.core.PlayerBridge
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Observable UI state fed by the core's events. */
class AppModel {
    /** Latest `Core::snapshot`, refreshed on every StateChanged event. */
    val snapshot = MutableStateFlow<dev.yoa.bilimusic.core.Snapshot?>(null)

    /** Decoded cover keyed by `bvid#cid#page`; UI correlates with snapshot.coverKey. */
    val cover = MutableStateFlow<Pair<String, ImageBitmap?>?>(null)
}

/** Forwards ExoPlayer commands coming from the Rust core. */
class ExoPlayerBridge : PlayerBridge {

    private data class LoadRequest(val url: String, val userAgent: String, val referer: String)

    private val mainHandler = Handler(Looper.getMainLooper())

    @Volatile
    private var controller: MediaController? = null

    @Volatile
    private var pendingLoad: LoadRequest? = null

    fun attach(controller: MediaController) {
        // MediaController is main-thread-only; Rust calls arrive on tokio
        // workers, so everything funnels through the main handler.
        mainHandler.post {
            this.controller = controller
            pendingLoad?.let { request ->
                pendingLoad = null
                applyLoad(request)
            }
        }
    }

    private fun applyLoad(request: LoadRequest) {
        val controller = controller ?: return
        // UA/Referer are configured on the service's DefaultHttpDataSource
        // factory (media3 1.11 has no per-item headers); the values carried
        // by LoadRequest are informational and match those constants.
        val mediaItem = MediaItem.Builder().setUri(request.url).build()
        controller.setMediaItem(mediaItem)
        controller.prepare()
        controller.play()
    }

    override fun load(url: String, userAgent: String, referer: String) {
        val request = LoadRequest(url, userAgent, referer)
        mainHandler.post {
            if (controller == null) {
                // The service/controller is still connecting; queue the first track.
                pendingLoad = request
            } else {
                applyLoad(request)
            }
        }
    }

    override fun togglePause() {
        mainHandler.post { controller?.let { it.playWhenReady = !it.playWhenReady } }
    }

    override fun seekBy(seconds: Double) {
        mainHandler.post {
            controller?.let {
                it.seekTo((it.currentPosition + seconds * 1000).coerceAtLeast(0.0).toLong())
            }
        }
    }

    override fun seekTo(seconds: Double) {
        mainHandler.post { controller?.seekTo((seconds * 1000).toLong().coerceAtLeast(0L)) }
    }

    // Core volume is on a 0-150 scale (100 = default); ExoPlayer is 0-1.
    override fun setVolume(volume: Double) {
        mainHandler.post {
            controller?.volume = (volume / 150.0).toFloat().coerceIn(0f, 1f)
        }
    }

    override fun stop() {
        mainHandler.post {
            controller?.let {
                it.stop()
                it.clearMediaItems()
            }
        }
    }
}

/** Pushes session events from tokio worker threads into the Compose-friendly model. */
class ObserverRelay(
    private val coreProvider: () -> Core,
    private val model: AppModel,
) : CoreObserver {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onEvent(event: CoreEvent) {
        scope.launch {
            try {
                when (event) {
                    is CoreEvent.StateChanged -> {
                        model.snapshot.value = coreProvider().snapshot()
                    }
                    is CoreEvent.CoverReady -> {
                        val bitmap = withContext(Dispatchers.Default) {
                            try {
                                BitmapFactory.decodeByteArray(
                                    event.bytes,
                                    0,
                                    event.bytes.size,
                                )?.asImageBitmap()
                            } catch (error: Exception) {
                                Log.e("BiliMusic", "封面解码失败", error)
                                null
                            }
                        }
                        model.cover.value = event.key to bitmap
                    }
                }
            } catch (error: Exception) {
                Log.e("BiliMusic", "处理核心事件失败", error)
            }
        }
    }
}

/** Process-wide singletons: the Rust core and the controller connection. */
object CoreHolder {

    lateinit var core: Core
        private set

    val model = AppModel()
    val bridge = ExoPlayerBridge()

    /** Non-null once the MediaController to PlaybackService is connected. */
    val controllerFlow = MutableStateFlow<MediaController?>(null)

    fun init(context: Context): AppModel {
        if (::core.isInitialized) {
            return model
        }
        core = Core(
            config = CoreConfig(
                dataDir = context.filesDir.resolve("bili-music").absolutePath,
                cacheDir = context.cacheDir.resolve("bili-music").absolutePath,
                cacheMb = 1024u,
            ),
            playerBridge = bridge,
            observer = ObserverRelay({ core }, model),
        )
        return model
    }

    /** Start PlaybackService and connect a MediaController to it. */
    fun connectController(context: Context) {
        if (controllerFlow.value != null) {
            return
        }
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val future = MediaController.Builder(context, token).buildAsync()
        future.addListener({
            try {
                val controller = future.get()
                bridge.attach(controller)
                controllerFlow.value = controller
            } catch (error: Exception) {
                Log.e("BiliMusic", "连接 MediaController 失败", error)
            }
        }, ContextCompat.getMainExecutor(context))
    }
}
