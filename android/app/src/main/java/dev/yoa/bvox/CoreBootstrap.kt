package dev.yoa.bvox

import android.content.ComponentName
import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import dev.yoa.bvox.core.Core
import dev.yoa.bvox.core.CoreConfig
import dev.yoa.bvox.core.CoreEvent
import dev.yoa.bvox.core.CoreObserver
import dev.yoa.bvox.core.PlayerBridge
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/** Observable UI state fed by the core's events. */
class AppModel {
    /** Latest `Core::snapshot`, refreshed on every StateChanged event. */
    val snapshot = MutableStateFlow<dev.yoa.bvox.core.Snapshot?>(null)

    /** Decoded cover keyed by `bvid#cid#page`; UI correlates with snapshot.coverKey. */
    val cover = MutableStateFlow<Pair<String, ImageBitmap?>?>(null)
}

/** Forwards ExoPlayer commands coming from the Rust core. */
class ExoPlayerBridge : PlayerBridge {

    private data class LoadRequest(
        val url: String,
        val userAgent: String,
        val referer: String,
        val title: String,
        val artist: String,
        val coverUrl: String?,
        val startPosition: Double,
    )

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
        // The core hands over cached tracks as plain filesystem paths —
        // give them an explicit file:// scheme so DefaultDataSource routes
        // them to FileDataSource instead of the HTTP stack.
        val uri = if (request.url.startsWith("http://") || request.url.startsWith("https://")) {
            Uri.parse(request.url)
        } else {
            Uri.fromFile(File(request.url))
        }
        // The media notification and lock screen render these fields. Without
        // them Media3 falls back to the app label, so the card looks like a
        // generic foreground-service notice instead of a music player.
        val metadata = MediaMetadata.Builder()
            .setTitle(request.title)
            .setArtist(request.artist)
            // The lyric timeline swaps `artist` for the current line so lock
            // screen / QS media controls show lyrics too. Keep the real artist
            // here so it can be restored at any time.
            .setAlbumArtist(request.artist)
            .setAlbumTitle("Bvox")
            .build()
        val mediaItem = MediaItem.Builder()
            .setUri(uri)
            .setMediaMetadata(
                if (request.coverUrl.isNullOrBlank()) {
                    metadata
                } else {
                    // The service's bitmap loader carries the same UA/Referer
                    // as the audio stack, so Bilibili's CDN accepts it.
                    metadata.buildUpon().setArtworkUri(Uri.parse(request.coverUrl)).build()
                }
            )
            .build()
        controller.setMediaItem(mediaItem)
        controller.prepare()
        // Media3 keeps a seek made before the timeline is ready and applies it
        // once the item is prepared; this is how a restored session resumes.
        if (request.startPosition > 0.0) {
            controller.seekTo((request.startPosition * 1000.0).toLong())
        }
        controller.play()
    }

    override fun load(
        url: String,
        userAgent: String,
        referer: String,
        title: String,
        artist: String,
        coverUrl: String?,
        startPosition: Double,
    ) {
        val request = LoadRequest(url, userAgent, referer, title, artist, coverUrl, startPosition)
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
                                Log.e("Bvox", "封面解码失败", error)
                                null
                            }
                        }
                        model.cover.value = event.key to bitmap
                    }
                }
            } catch (error: Exception) {
                Log.e("Bvox", "处理核心事件失败", error)
            }
        }
    }
}

/** Process-wide singletons: the Rust core and the controller connection. */
object CoreHolder {

    lateinit var core: Core
        private set

    /** False when the playback service runs without MainActivity having initialized the core. */
    val isReady: Boolean
        get() = ::core.isInitialized

    val model = AppModel()
    val bridge = ExoPlayerBridge()

    /** Non-null once the MediaController to PlaybackService is connected. */
    val controllerFlow = MutableStateFlow<MediaController?>(null)

    /**
     * Current lyric step published by `PlaybackService`, shared with the
     * floating-overlay service. Null before the first line or with no media.
     */
    val lyricLine = MutableStateFlow<LyricUiLine?>(null)

    fun init(context: Context): AppModel {
        if (::core.isInitialized) {
            return model
        }
        core = Core(
            config = CoreConfig(
                dataDir = context.filesDir.resolve("bvox").absolutePath,
                cacheDir = context.cacheDir.resolve("bvox").absolutePath,
                cacheMb = CachePrefs.limitMb(context).toUInt(),
                cacheMode = CachePrefs.mode(context),
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
                Log.e("Bvox", "连接 MediaController 失败", error)
            }
        }, ContextCompat.getMainExecutor(context))
    }
}
