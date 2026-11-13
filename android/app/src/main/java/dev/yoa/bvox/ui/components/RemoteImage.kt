package dev.yoa.bvox.ui.components

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

/** Small in-memory cache so tab switches do not re-download avatars. */
private val remoteImageCache = LruCache<String, ImageBitmap>(24)

/**
 * Load an avatar from a Bilibili CDN URL. URLs are normalized to https and
 * fetched with a browser UA/Referer because the CDN rejects bare requests.
 */
@Composable
fun rememberRemoteImage(url: String?): ImageBitmap? {
    val normalized = url
        ?.trim()
        ?.takeIf { it.isNotEmpty() }
        ?.replaceFirst("http://", "https://")
    val image by produceState(
        initialValue = normalized?.let(remoteImageCache::get),
        key1 = normalized,
    ) {
        if (normalized == null) return@produceState
        val cached = remoteImageCache.get(normalized)
        if (cached != null) {
            value = cached
            return@produceState
        }
        value = withContext(Dispatchers.IO) { downloadImage(normalized) }
        value?.let { remoteImageCache.put(normalized, it) }
    }
    return image
}

private fun downloadImage(url: String): ImageBitmap? = runCatching {
    val connection = URL(url).openConnection() as HttpURLConnection
    try {
        connection.connectTimeout = 8000
        connection.readTimeout = 8000
        connection.instanceFollowRedirects = true
        connection.setRequestProperty(
            "User-Agent",
            "Mozilla/5.0 (Linux; Android 16; 23127PN0CC) " +
                "AppleWebKit/537.36 (KHTML, like Gecko) " +
                "Chrome/155.0.0.0 Mobile Safari/537.36",
        )
        connection.setRequestProperty("Referer", "https://www.bilibili.com/")
        connection.inputStream.use { stream ->
            BitmapFactory.decodeStream(stream)?.asImageBitmap()
        }
    } finally {
        connection.disconnect()
    }
}.getOrNull()
