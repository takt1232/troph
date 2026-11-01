package com.sixdaggers.pocketfiles

import android.content.Context
import android.media.MediaScannerConnection
import android.util.Log

object MediaScannerBridge {
    private const val TAG = "Troph"

    @JvmStatic
    fun scan(context: Context, path: String) {
        try {
            MediaScannerConnection.scanFile(context, arrayOf(path), null, null)
        } catch (t: Throwable) {
            Log.w(TAG, "media scan failed for $path", t)
        }
    }
}
