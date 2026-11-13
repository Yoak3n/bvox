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

/** Small in-memory cache so tab switches and list scrolling do not re-download images. */
private val remoteImageCache = LruCache<String, ImageBitmap>(64)

/**
 * Ask the CDN for a list-sized thumbnail instead of the full cover. Covers are
 * 16:9 stills; a 320px wide version stays sharp in the square row artwork even
 * on 3x screens, while keeping the decode cost small.
 */
internal fun listThumbnailUrl(url: String?): String? = url
    ?.trim()
    ?.takeIf { it.isNotEmpty() }
    ?.replaceFirst("http://", "https://")
    ?.let { normalized ->
        if (!normalized.contains("hdslb.com") || normalized.contains('@')) {
            normalized
        } else {
            val (base, query) = normalized.split('?', limit = 2)
                .let { parts -> parts[0] to parts.getOrNull(1) }
            val thumbnail = "$base@320w_180h.jpg"
            if (query == null) thumbnail else "$thumbnail?$query"
        }
    }

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
