package com.sixdaggers.pocketfiles

import android.content.Context
import android.graphics.Bitmap
import android.util.Log
import android.util.Size
import android.media.ThumbnailUtils
import java.io.ByteArrayOutputStream
import java.io.File

object ThumbnailBridge {
    private const val TAG = "Troph"

    @JvmStatic
    @Suppress("UNUSED_PARAMETER")
    fun thumbnail(context: Context, path: String, size: Int, isVideo: Boolean): ByteArray? {
        if (size <= 0) return null
        return try {
            val file = File(path)
            if (!file.isFile) return null
            val dimension = Size(size, size)
            val bitmap: Bitmap? = if (isVideo) {
                ThumbnailUtils.createVideoThumbnail(file, dimension, null)
            } else {
                ThumbnailUtils.createImageThumbnail(file, dimension, null)
            }
            bitmap?.let { image ->
                try {
                    val output = ByteArrayOutputStream()
                    val quality = if (isVideo) 78 else 84
                    image.compress(Bitmap.CompressFormat.WEBP_LOSSY, quality, output)
                    output.toByteArray()
                } finally {
                    image.recycle()
                }
            }
        } catch (t: Throwable) {
            Log.w(TAG, "thumbnail failed for $path", t)
            null
        }
    }
}
