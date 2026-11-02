package com.sixdaggers.pocketfiles

import android.content.Context

object ServerBridge {
    init {
        System.loadLibrary("pocketfiles")
    }

    @JvmStatic
    external fun startServer(context: Context, configJson: String): String

    @JvmStatic
    external fun restartServer(context: Context, configJson: String): String

    @JvmStatic
    external fun stopServer(): String

    @JvmStatic
    external fun transfersJson(): String

    @JvmStatic
    external fun isRunning(): Boolean
}
