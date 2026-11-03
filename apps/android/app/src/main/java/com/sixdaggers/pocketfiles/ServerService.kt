package com.sixdaggers.pocketfiles

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.storage.StorageManager
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.SecureRandom
import java.util.Locale

class ServerService : Service() {
    data class Transfer(
        val id: String,
        val name: String,
        val loaded: Long,
        val total: Long?,
    )

    data class State(
        val running: Boolean,
        val starting: Boolean,
        val url: String?,
        val pin: String?,
        val hostname: String?,
        val error: String?,
        val transfers: List<Transfer> = emptyList(),
    )

    companion object {
        const val ACTION_START = "com.sixdaggers.pocketfiles.START"
        const val ACTION_RESTART = "com.sixdaggers.pocketfiles.RESTART"
        const val ACTION_STOP = "com.sixdaggers.pocketfiles.STOP"
        const val ACTION_CANCEL_TRANSFERS = "com.sixdaggers.pocketfiles.CANCEL_TRANSFERS"
        const val EXTRA_PORT = "port"
        private const val CHANNEL_ID = "pocketfiles-server"
        private const val NOTIFICATION_ID = 1
        private const val TAG = "Troph"

        @Volatile
        var state: State? = null
            private set
    }

    private val handler = Handler(Looper.getMainLooper())
    private val transferTicker = object : Runnable {
        override fun run() {
            if (!running) return
            refreshTransfers()
            handler.postDelayed(this, 1000)
        }
    }
    private val nativeLock = Any()
    private var running = false
    private var generation = 0
    private var pin = ""
    private var port = 55432
    private var serverHostname: String? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var multicastLock: WifiManager.MulticastLock? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                port = intent.getIntExtra(EXTRA_PORT, 55432)
                startServer()
            }
            ACTION_RESTART -> restartServer()
            ACTION_STOP -> stopServerAndSelf()
            ACTION_CANCEL_TRANSFERS -> cancelAllTransfers()
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        stopTransferTicker()
        running = false
        generation += 1
        synchronized(nativeLock) {
            try {
                ServerBridge.stopServer()
            } catch (t: Throwable) {
                Log.w(TAG, "stopServer failed", t)
            }
        }
        releaseLocks()
        state = null
        super.onDestroy()
    }

    private fun startServer() {
        if (running || state?.starting == true) return
        if (!Environment.isExternalStorageManager()) {
            state = State(false, false, null, null, null, "All files access is required.")
            stopSelf()
            return
        }

        pin = generatePin()
        launchServer(restart = false)
    }

    private fun restartServer() {
        val current = state
        if (current?.running != true && current?.starting != true) return
        if (pin.isBlank()) pin = generatePin()
        launchServer(restart = true)
    }

    private fun launchServer(restart: Boolean) {
        stopTransferTicker()
        val previous = if (restart) state else null
        val launchGeneration = ++generation
        state = State(false, true, previous?.url, pin, previous?.hostname, null)
        startForeground(
            NOTIFICATION_ID,
            buildNotification(
                if (restart) "Restarting Troph\u2026" else "Starting Troph\u2026",
                null,
            ),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
        )
        if (wakeLock == null) acquireLocks()

        val config = buildConfig()
        Thread {
            val result: String? = try {
                synchronized(nativeLock) {
                    when {
                        launchGeneration != generation -> null
                        restart -> ServerBridge.restartServer(applicationContext, config)
                        else -> ServerBridge.startServer(applicationContext, config)
                    }
                }
            } catch (t: Throwable) {
                Log.e(TAG, if (restart) "native restart failed" else "native start failed", t)
                JSONObject().put("ok", false).put("error", t.message ?: t.toString()).toString()
            }
            if (result == null) return@Thread
            var parsed: JSONObject? = null
            try {
                parsed = JSONObject(result)
            } catch (t: Throwable) {
                Log.e(TAG, "bad server response: $result", t)
            }
            handler.post {
                if (launchGeneration != generation) return@post
                if (parsed?.optBoolean("ok") == true) {
                    running = true
                    val url = parsed.optString("url").ifBlank { null }
                    val hostname = parsed.optString("hostname").ifBlank { null }
                    state = State(true, false, url, pin, hostname, null)
                    updateNotification()
                    startTransferTicker()
                } else {
                    state = State(
                        false,
                        false,
                        null,
                        null,
                        null,
                        parsed?.optString("error")?.ifBlank { null } ?: "Server failed to start.",
                    )
                    running = false
                    releaseLocks()
                    stopForeground(STOP_FOREGROUND_REMOVE)
                    stopSelf()
                }
            }
        }.start()
    }

    private fun stopServerAndSelf() {
        if (!running && state?.starting != true) {
            stopSelf()
            return
        }
        generation += 1
        stopTransferTicker()
        state = State(false, false, null, pin, null, null)
        Thread {
            synchronized(nativeLock) {
                try {
                    ServerBridge.stopServer()
                } catch (t: Throwable) {
                    Log.w(TAG, "stopServer failed", t)
                }
            }
            handler.post {
                running = false
                releaseLocks()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
        }.start()
    }

    private fun buildConfig(): String {
        val roots = JSONArray()
        if (ShareStore.mode(this) == ShareStore.Mode.EVERYTHING) {
            addEverythingRoots(roots)
        } else {
            for (share in ShareStore.shares(this)) {
                val file = File(share.path)
                val kind = when {
                    file.isDirectory -> "folder"
                    file.isFile -> "file"
                    else -> continue
                }
                roots.put(
                    JSONObject()
                        .put("label", share.label)
                        .put("path", file.absolutePath)
                        .put("kind", kind),
                )
            }
        }

        val model = Build.MODEL.ifBlank { "phone" }
            .lowercase(Locale.US)
            .replace(Regex("[^a-z0-9]+"), "-")
            .trim('-')
            .take(32)
        val hostname = serverHostname ?: run {
            val suffix = String.format(Locale.US, "%04x", SecureRandom().nextInt(0x10000))
            "troph-$model-$suffix.local".also { serverHostname = it }
        }

        return JSONObject()
            .put("port", port)
            .put("pin", pin)
            .put("roots", roots)
            .put("cache_dir", cacheDir.absolutePath)
            .put("device_name", Build.MODEL)
            .put("hostname", hostname)
            .toString()
    }

    private fun addEverythingRoots(roots: JSONArray) {
        val primary = Environment.getExternalStorageDirectory()
        roots.put(
            JSONObject()
                .put("label", "Internal storage")
                .put("path", primary.absolutePath)
                .put("kind", "folder"),
        )

        val sm = getSystemService(StorageManager::class.java)
        for (volume in sm.storageVolumes) {
            val dir: File = volume.directory ?: continue
            if (dir.absolutePath == primary.absolutePath) continue
            val label = try {
                volume.getDescription(this)
            } catch (_: Throwable) {
                "SD card"
            }
            roots.put(
                JSONObject()
                    .put("label", label)
                    .put("path", dir.absolutePath)
                    .put("kind", "folder"),
            )
        }
    }

    private fun generatePin(): String {
        val n = SecureRandom().nextInt(1_000_000)
        return String.format(Locale.US, "%06d", n)
    }

    private fun startTransferTicker() {
        handler.removeCallbacks(transferTicker)
        handler.post(transferTicker)
    }

    private fun stopTransferTicker() {
        handler.removeCallbacks(transferTicker)
    }

    private fun refreshTransfers() {
        val current = state ?: return
        val transfers = readTransfers()
        if (transfers != current.transfers) {
            state = current.copy(transfers = transfers)
            updateNotification()
        }
    }

    private fun cancelAllTransfers() {
        try {
            ServerBridge.cancelAllTransfers()
        } catch (t: Throwable) {
            Log.w(TAG, "cancelAllTransfers failed", t)
        }
        refreshTransfers()
    }

    private fun readTransfers(): List<Transfer> {
        val raw = try {
            ServerBridge.transfersJson()
        } catch (t: Throwable) {
            Log.w(TAG, "cannot read transfers", t)
            return emptyList()
        }
        return try {
            val array = JSONArray(raw)
            buildList {
                for (index in 0 until array.length()) {
                    val item = array.optJSONObject(index) ?: continue
                    val id = item.optString("id")
                    val name = item.optString("name")
                    if (id.isBlank() || name.isBlank()) continue
                    val total = if (item.has("total") && !item.isNull("total")) {
                        item.optLong("total")
                    } else {
                        null
                    }
                    add(Transfer(id, name, item.optLong("loaded"), total))
                }
            }
        } catch (t: Throwable) {
            Log.w(TAG, "bad transfer payload", t)
            emptyList()
        }
    }

    private fun createNotificationChannel() {
        val nm = getSystemService(NotificationManager::class.java)
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Troph server",
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Shows that the Troph server is running on your local network."
            setShowBadge(false)
        }
        nm.createNotificationChannel(channel)
    }

    private fun buildNotification(
        title: String,
        text: String?,
        transfers: List<Transfer> = emptyList(),
    ): Notification {
        val openIntent = PendingIntent.getActivity(
            this,
            1,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val stopIntent = PendingIntent.getService(
            this,
            2,
            Intent(this, ServerService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle(title)
            .setContentIntent(openIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .addAction(
                Notification.Action.Builder(
                    null,
                    "Stop",
                    stopIntent,
                ).build(),
            )
        if (text != null) builder.setContentText(text)
        if (transfers.isNotEmpty()) {
            val cancelIntent = PendingIntent.getService(
                this,
                3,
                Intent(this, ServerService::class.java).setAction(ACTION_CANCEL_TRANSFERS),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            builder.addAction(
                Notification.Action.Builder(
                    null,
                    "Cancel transfers",
                    cancelIntent,
                ).build(),
            )
            val total = transfers.mapNotNull { it.total }.sum()
            val loaded = transfers.sumOf { it.loaded }
            if (transfers.all { it.total != null && it.total > 0 } && total > 0) {
                val progress = ((loaded * 100) / total).toInt().coerceIn(0, 100)
                builder.setProgress(100, progress, false)
            } else {
                builder.setProgress(0, 0, true)
            }
        }
        return builder.build()
    }

    private fun updateNotification() {
        val current = state
        val transfers = current?.transfers.orEmpty()
        val text = when {
            transfers.isNotEmpty() -> transferSummary(transfers)
            current?.running == true -> current.url ?: "Server running"
            else -> "Server running"
        }
        val nm = getSystemService(NotificationManager::class.java)
        nm.notify(NOTIFICATION_ID, buildNotification("Troph server", text, transfers))
    }

    private fun transferSummary(transfers: List<Transfer>): String {
        val percent = transferPercent(transfers)
        if (transfers.size == 1) {
            val name = transfers[0].name
            return if (percent == null) "Receiving $name" else "Receiving $name \u00b7 $percent%"
        }
        val label = "Receiving ${transfers.size} files"
        return if (percent == null) label else "$label \u00b7 $percent%"
    }

    private fun transferPercent(transfers: List<Transfer>): Int? {
        if (transfers.isEmpty() || transfers.any { it.total == null || it.total <= 0 }) {
            return null
        }
        val total = transfers.sumOf { it.total ?: 0 }
        if (total <= 0) return null
        val loaded = transfers.sumOf { it.loaded }
        return ((loaded * 100) / total).toInt().coerceIn(0, 100)
    }

    @Suppress("DEPRECATION")
    private fun acquireLocks() {
        val pm = getSystemService(PowerManager::class.java)
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Troph:server").apply {
            setReferenceCounted(false)
            acquire()
        }
        val wm = applicationContext.getSystemService(WifiManager::class.java)
        wifiLock = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "Troph:wifi").apply {
            setReferenceCounted(false)
            acquire()
        }
        multicastLock = wm.createMulticastLock("Troph:mdns").apply {
            setReferenceCounted(false)
            acquire()
        }
    }

    private fun releaseLocks() {
        try {
            wakeLock?.takeIf { it.isHeld }?.release()
        } catch (_: Throwable) {
        }
        wakeLock = null
        try {
            wifiLock?.takeIf { it.isHeld }?.release()
        } catch (_: Throwable) {
        }
        wifiLock = null
        try {
            multicastLock?.takeIf { it.isHeld }?.release()
        } catch (_: Throwable) {
        }
        multicastLock = null
    }
}
