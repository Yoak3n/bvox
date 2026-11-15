package dev.yoa.bvox.ui.components

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import dev.yoa.bvox.core.CoreHolder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Small in-memory cache so tab switches and list scrolling do not re-download images. */
private val remoteImageCache = LruCache<String, ImageBitmap>(64)

/**
 * Longest side of a decoded image. Rows draw 44dp art (~132px on a 3x screen),
 * so a full 640×360 cover would cost ~900 KB per LRU entry; downsampling keeps
 * the bitmap near 320×180 instead.
 */
private const val MAX_DECODED_PX = 360

/**
 * Load a cover or avatar from a Bilibili CDN URL. The download runs in the
 * core's HTTP stack (CDN probing, retry, on-disk cover cache) because the CDN
 * has nodes that accept DNS but drop connections; a bare `HttpURLConnection`
 * would fail on those and leave every row on the placeholder tile.
 *
 * Covers keep their original URL: the core asks the image service for the same
 * 640w thumbnail the player and notification use, so one cache file serves
 * every view and starting playback finds the cover on disk.
 */
@Composable
internal fun rememberRemoteImage(url: String?): ImageBitmap? {
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

/** Blocking: `Core::fetch_thumbnail` only returns after the request settles. */
private fun downloadImage(url: String): ImageBitmap? {
    if (!CoreHolder.isReady) return null
    val bytes = CoreHolder.core.fetchThumbnail(url) ?: return null
    return decodeThumbnail(bytes)
}

/** Decode a cover at the largest power-of-two downsample that still covers [MAX_DECODED_PX]. */
private fun decodeThumbnail(bytes: ByteArray): ImageBitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    var sample = 1
    while (maxOf(bounds.outWidth, bounds.outHeight) / sample > MAX_DECODED_PX) {
        sample *= 2
    }
    val options = BitmapFactory.Options().apply { inSampleSize = sample }
    return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
}
