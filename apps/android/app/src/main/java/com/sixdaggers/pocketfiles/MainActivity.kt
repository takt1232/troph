package com.sixdaggers.pocketfiles

import android.Manifest
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.ColorStateList
import android.graphics.Bitmap
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.widget.Button
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import java.io.File

class MainActivity : Activity() {
    private val handler = Handler(Looper.getMainLooper())
    private lateinit var statusView: TextView
    private lateinit var statusDot: View
    private lateinit var errorView: TextView
    private lateinit var urlView: TextView
    private lateinit var pinView: TextView
    private lateinit var hostnameView: TextView
    private lateinit var qrView: ImageView
    private lateinit var serverButton: Button
    private lateinit var copyUrlButton: Button
    private lateinit var copyPinButton: Button
    private lateinit var openButton: Button
    private lateinit var filesRow: SettingRow
    private lateinit var notificationRow: SettingRow
    private lateinit var sharingSummaryView: TextView
    private lateinit var credentialsGroup: LinearLayout
    private lateinit var transfersPanel: LinearLayout
    private lateinit var transfersTitle: TextView
    private lateinit var transfersList: LinearLayout
    private var lastQrText: String? = null

    private val ticker = object : Runnable {
        override fun run() {
            updateUi()
            handler.postDelayed(this, 1000)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setDecorFitsSystemWindows(false)
        setContentView(buildContent())
        updateUi()
    }

    override fun onResume() {
        super.onResume()
        handler.removeCallbacks(ticker)
        handler.post(ticker)
    }

    override fun onPause() {
        super.onPause()
        handler.removeCallbacks(ticker)
    }

    private fun buildContent(): View {
        val root = ScrollView(this).apply {
            setBackgroundColor(TrophTheme.BG)
            isFillViewport = true
            isVerticalScrollBarEnabled = false
        }
        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(20), dp(24), dp(20), dp(32))
        }
        root.addView(content)
        root.setOnApplyWindowInsetsListener { view, insets ->
            val bars = insets.getInsets(WindowInsets.Type.systemBars())
            view.setPadding(0, bars.top, 0, bars.bottom)
            insets
        }

        content.addView(label("Troph", 28f, TrophTheme.TEXT, bold = true))
        content.addView(
            label("Your phone's files, in your computer's browser.", 14f, TrophTheme.MUTED)
                .apply { setPadding(0, dp(4), 0, dp(20)) },
        )

        val serverPanel = panel()
        val statusRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        statusDot = View(this).apply {
            background = dot(TrophTheme.MUTED)
            layoutParams = LinearLayout.LayoutParams(dp(8), dp(8)).apply { marginEnd = dp(10) }
        }
        statusRow.addView(statusDot)
        statusView = label("Server stopped", 18f, TrophTheme.TEXT, bold = true)
        statusRow.addView(statusView)
        serverPanel.addView(statusRow)

        errorView = label("", 13f, TrophTheme.DANGER).apply {
            setPadding(0, dp(8), 0, 0)
            visibility = View.GONE
        }
        serverPanel.addView(errorView)

        serverButton = primaryButton("Start server") { startServer() }
        serverPanel.addView(
            serverButton,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(16) },
        )

        credentialsGroup = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            visibility = View.GONE
        }
        credentialsGroup.addView(rule(18, 0))
        credentialsGroup.addView(
            label("Open this address on your computer", 13f, TrophTheme.MUTED)
                .apply { setPadding(0, dp(16), 0, 0) },
        )
        urlView = label("", 17f, TrophTheme.ACCENT, mono = true).apply {
            setTextIsSelectable(true)
            setPadding(0, dp(4), 0, dp(6))
        }
        credentialsGroup.addView(urlView)
        hostnameView = label("", 12f, TrophTheme.MUTED, mono = true).apply {
            setTextIsSelectable(true)
        }
        credentialsGroup.addView(hostnameView)

        credentialsGroup.addView(
            label("PIN", 13f, TrophTheme.MUTED).apply { setPadding(0, dp(16), 0, 0) },
        )
        pinView = label("", 32f, TrophTheme.TEXT, bold = true, mono = true).apply {
            setTextIsSelectable(true)
            letterSpacing = 0.12f
        }
        credentialsGroup.addView(pinView)

        val credentialButtons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(0, dp(12), 0, 0)
        }
        copyUrlButton = smallButton("Copy address") { copy(urlView.text.toString(), "Address") }
        copyPinButton = smallButton("Copy PIN") { copy(pinView.text.toString(), "PIN") }
        credentialButtons.addView(copyUrlButton, rowParams())
        credentialButtons.addView(copyPinButton, rowParams().apply { marginEnd = 0 })
        credentialsGroup.addView(credentialButtons)

        qrView = ImageView(this).apply {
            adjustViewBounds = true
            setBackgroundColor(Color.WHITE)
            setPadding(dp(8), dp(8), dp(8), dp(8))
        }
        credentialsGroup.addView(
            qrView,
            LinearLayout.LayoutParams(dp(180), dp(180)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                topMargin = dp(16)
            },
        )
        openButton = secondaryButton("Open in this phone's browser") { openUrl() }
        credentialsGroup.addView(
            openButton,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(14) },
        )
        serverPanel.addView(credentialsGroup)
        content.addView(serverPanel)

        transfersPanel = panel().apply { visibility = View.GONE }
        val transfersHead = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        transfersHead.addView(
            View(this).apply {
                background = dot(TrophTheme.ACCENT)
                layoutParams = LinearLayout.LayoutParams(dp(8), dp(8)).apply { marginEnd = dp(10) }
            },
        )
        transfersTitle = label("Receiving", 15f, TrophTheme.TEXT, bold = true)
        transfersHead.addView(transfersTitle)
        transfersPanel.addView(transfersHead)
        transfersList = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        transfersPanel.addView(transfersList)
        content.addView(
            transfersPanel,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(14) },
        )

        content.addView(
            sectionTitle("Setup"),
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(26) },
        )
        content.addView(
            label(
                "All files access is required to read the folders and files you share. The notification and battery opt-out keep transfers alive while the screen is off.",
                13f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(6), 0, dp(6)) },
        )
        val setupPanel = panel()
        filesRow = SettingRow(this, "All files access")
        setupPanel.addView(filesRow)
        setupPanel.addView(rule())
        notificationRow = SettingRow(this, "Notifications")
        setupPanel.addView(notificationRow)
        setupPanel.addView(rule())
        setupPanel.addView(
            SettingRow(this, "Battery optimization").apply {
                action("Open settings") { openBatterySettings() }
            },
        )
        content.addView(setupPanel)

        content.addView(
            sectionTitle("Shared items"),
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(26) },
        )
        sharingSummaryView = label("", 14f, TrophTheme.MUTED).apply { setPadding(0, dp(6), 0, 0) }
        content.addView(sharingSummaryView)
        content.addView(
            secondaryButton("Manage shared items") { openSharing() },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(12) },
        )
        content.addView(
            label(
                "Only the folders and files you choose are visible to the browser. Changes restart the server automatically and keep the same PIN.",
                12f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(10), 0, 0) },
        )

        content.addView(rule(28, 0))
        content.addView(
            label(
                "The server is available only on your local network and is protected by a PIN. Keep the phone connected to Wi-Fi.",
                12f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(16), 0, 0) },
        )
        content.addView(
            label("Troph v${BuildConfig.VERSION_NAME}", 12f, TrophTheme.MUTED).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                setPadding(0, dp(20), 0, dp(4))
            },
        )

        return root
    }

    private fun updateUi() {
        val state = ServerService.state
        val running = state?.running == true
        val starting = state?.starting == true
        when {
            starting -> {
                statusView.text = "Starting server\u2026"
                statusView.setTextColor(TrophTheme.ACCENT)
                statusDot.background = dot(TrophTheme.ACCENT)
            }
            running -> {
                statusView.text = "Server running"
                statusView.setTextColor(TrophTheme.ACCENT)
                statusDot.background = dot(TrophTheme.ACCENT)
            }
            else -> {
                statusView.text = "Server stopped"
                statusView.setTextColor(TrophTheme.TEXT)
                statusDot.background = dot(TrophTheme.MUTED)
            }
        }
        errorView.visibility = if (!state?.error.isNullOrBlank()) View.VISIBLE else View.GONE
        errorView.text = state?.error.orEmpty()

        serverButton.text = if (running || starting) "Stop server" else "Start server"
        serverButton.setOnClickListener {
            if (ServerService.state?.running == true || ServerService.state?.starting == true) {
                stopServer()
            } else {
                startServer()
            }
        }

        copyUrlButton.isEnabled = !state?.url.isNullOrBlank()
        copyPinButton.isEnabled = !state?.pin.isNullOrBlank()
        openButton.isEnabled = !state?.url.isNullOrBlank()

        if (state?.pin != null || state?.url != null) {
            credentialsGroup.visibility = View.VISIBLE
            urlView.text = state?.url ?: ""
            hostnameView.text = state?.hostname?.let { "http://$it:${port()}" } ?: ""
            pinView.text = state?.pin ?: ""
            val qrText = if (state?.url != null && state?.pin != null) {
                "${state.url}/?pin=${state.pin}"
            } else {
                null
            }
            if (qrText != lastQrText) {
                lastQrText = qrText
                qrView.setImageBitmap(qrText?.let { makeQr(it) })
            }
        } else {
            credentialsGroup.visibility = View.GONE
            lastQrText = null
        }

        val allFiles = Environment.isExternalStorageManager()
        if (allFiles) {
            filesRow.granted("Granted")
        } else {
            filesRow.action("Allow") { openAllFilesSettings() }
        }

        val notifications = Build.VERSION.SDK_INT < 33 ||
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        if (notifications) {
            notificationRow.granted("Allowed")
        } else {
            notificationRow.action("Allow") { requestNotificationPermission() }
        }
        renderTransfers(state?.transfers.orEmpty(), running)
        updateSharingSummary()
    }

    private fun renderTransfers(transfers: List<ServerService.Transfer>, running: Boolean) {
        val visible = running && transfers.isNotEmpty()
        transfersPanel.visibility = if (visible) View.VISIBLE else View.GONE
        if (!visible) {
            transfersList.removeAllViews()
            return
        }
        transfersTitle.text = if (transfers.size == 1) {
            "Receiving 1 file"
        } else {
            "Receiving ${transfers.size} files"
        }
        transfersList.removeAllViews()
        for (transfer in transfers) {
            transfersList.addView(transferRow(transfer))
        }
    }

    private fun transferRow(transfer: ServerService.Transfer): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, dp(10), 0, 0)
        }
        val nameRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        nameRow.addView(
            label(transfer.name, 12f, TrophTheme.MUTED).apply {
                maxLines = 1
                ellipsize = TextUtils.TruncateAt.MIDDLE
                layoutParams = LinearLayout.LayoutParams(
                    0,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    1f,
                )
            },
        )
        val percent = transfer.total
            ?.takeIf { it > 0 }
            ?.let { ((transfer.loaded * 100) / it).toInt().coerceIn(0, 100) }
        nameRow.addView(
            label(
                if (percent != null) "$percent%" else formatSize(transfer.loaded),
                12f,
                TrophTheme.MUTED,
            ),
        )
        row.addView(nameRow)
        val progress = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 100
            if (percent == null) {
                isIndeterminate = true
            } else {
                progress = percent
            }
            progressTintList = ColorStateList.valueOf(TrophTheme.ACCENT)
            progressBackgroundTintList = ColorStateList.valueOf(TrophTheme.BORDER)
            minimumHeight = dp(4)
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(4),
            ).apply { topMargin = dp(6) }
        }
        row.addView(progress)
        return row
    }

    private fun updateSharingSummary() {
        when (ShareStore.mode(this)) {
            ShareStore.Mode.EVERYTHING -> {
                sharingSummaryView.text =
                    "Everything on internal storage and SD or USB volumes is shared."
            }

            ShareStore.Mode.SELECTED -> {
                val shares = ShareStore.shares(this).filter { File(it.path).exists() }
                sharingSummaryView.text = when {
                    shares.isEmpty() ->
                        "Nothing is shared yet. The server will start with an empty library."
                    shares.size == 1 -> "1 item shared: ${shares[0].label}"
                    else -> "${shares.size} items shared."
                }
            }
        }
    }

    private fun openSharing() {
        startActivity(Intent(this, SharingActivity::class.java))
    }

    private fun port(): Int {
        val url = ServerService.state?.url ?: return 55432
        return try {
            Uri.parse(url).port.takeIf { it > 0 } ?: 55432
        } catch (_: Throwable) {
            55432
        }
    }

    private fun startServer() {
        if (!Environment.isExternalStorageManager()) {
            toast("Grant all files access first.")
            openAllFilesSettings()
            return
        }
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 10)
        }
        if (ShareStore.mode(this) == ShareStore.Mode.SELECTED &&
            ShareStore.shares(this).none { File(it.path).exists() }
        ) {
            toast("Nothing is shared yet. The browser will show an empty library.")
        }
        val intent = Intent(this, ServerService::class.java)
            .setAction(ServerService.ACTION_START)
            .putExtra(ServerService.EXTRA_PORT, 55432)
        startForegroundService(intent)
        handler.postDelayed({ updateUi() }, 200)
    }

    private fun stopServer() {
        startService(Intent(this, ServerService::class.java).setAction(ServerService.ACTION_STOP))
        handler.postDelayed({ updateUi() }, 300)
    }

    private fun openAllFilesSettings() {
        val intent = Intent(
            Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
            Uri.parse("package:$packageName"),
        )
        try {
            startActivity(intent)
        } catch (_: Throwable) {
            startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
        }
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= 33) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 10)
        } else {
            toast("Notifications are already available on this Android version.")
        }
    }

    private fun openBatterySettings() {
        try {
            startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        } catch (_: Throwable) {
            startActivity(Intent(Settings.ACTION_SETTINGS))
        }
    }

    private fun openUrl() {
        val state = ServerService.state ?: return
        val url = state.url ?: return
        val target = if (state.pin != null) "$url/?pin=${state.pin}" else url
        try {
            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(target)))
        } catch (_: Throwable) {
            toast("No browser available.")
        }
    }

    private fun copy(value: String, label: String) {
        if (value.isBlank()) return
        val clipboard = getSystemService(ClipboardManager::class.java)
        clipboard.setPrimaryClip(ClipData.newPlainText("Troph $label", value))
        toast("$label copied.")
    }

    private fun makeQr(content: String): Bitmap? {
        return try {
            val size = 512
            val matrix = QRCodeWriter().encode(content, BarcodeFormat.QR_CODE, size, size)
            val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
            for (x in 0 until size) {
                for (y in 0 until size) {
                    bitmap.setPixel(
                        x,
                        y,
                        if (matrix[x, y]) Color.BLACK else Color.WHITE,
                    )
                }
            }
            bitmap
        } catch (_: Throwable) {
            null
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        updateUi()
    }

    private fun toast(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
    }
}
