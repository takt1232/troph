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
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.widget.Button
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import java.io.File

class MainActivity : Activity() {
    private val handler = Handler(Looper.getMainLooper())
    private lateinit var statusView: TextView
    private lateinit var errorView: TextView
    private lateinit var urlView: TextView
    private lateinit var pinView: TextView
    private lateinit var hostnameView: TextView
    private lateinit var qrView: ImageView
    private lateinit var startButton: Button
    private lateinit var stopButton: Button
    private lateinit var copyUrlButton: Button
    private lateinit var copyPinButton: Button
    private lateinit var openButton: Button
    private lateinit var filesPermissionButton: Button
    private lateinit var notificationPermissionButton: Button
    private lateinit var batteryButton: Button
    private lateinit var sharingSummaryView: TextView
    private lateinit var manageSharingButton: Button
    private lateinit var credentialsCard: LinearLayout
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
        if (Build.VERSION.SDK_INT >= 30) {
            window.insetsController?.setSystemBarsAppearance(
                android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                    android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
                android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                    android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
            )
        }
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT
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
            setBackgroundColor(Color.rgb(246, 247, 249))
            isFillViewport = true
        }
        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(20), dp(20), dp(20), dp(28))
        }
        root.addView(content)
        root.setOnApplyWindowInsetsListener { view, insets ->
            val bars = insets.getInsets(WindowInsets.Type.systemBars())
            view.setPadding(0, bars.top, 0, bars.bottom)
            insets
        }

        content.addView(text("Troph", 30f, Color.rgb(17, 24, 39), bold = true))
        content.addView(
            text(
                "Your phone's files, in your computer's browser.",
                15f,
                Color.rgb(75, 85, 99),
            ).apply { setPadding(0, dp(4), 0, dp(18)) },
        )

        val statusCard = card()
        statusView = text("Server stopped", 18f, Color.rgb(17, 24, 39), bold = true)
        statusCard.addView(statusView)
        errorView = text("", 14f, Color.rgb(185, 28, 28)).apply {
            setPadding(0, dp(6), 0, 0)
            visibility = View.GONE
        }
        statusCard.addView(errorView)

        val buttonRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(0, dp(14), 0, 0)
        }
        startButton = button("Start server").apply {
            setOnClickListener { startServer() }
        }
        stopButton = button("Stop").apply {
            setOnClickListener { stopServer() }
        }
        buttonRow.addView(startButton, rowParams())
        buttonRow.addView(stopButton, rowParams())
        statusCard.addView(buttonRow)
        content.addView(statusCard)

        val sharingCard = card()
        sharingCard.addView(text("Shared items", 18f, Color.rgb(17, 24, 39), bold = true))
        sharingSummaryView = text("", 14f, Color.rgb(75, 85, 99)).apply {
            setPadding(0, dp(6), 0, 0)
        }
        sharingCard.addView(sharingSummaryView)
        manageSharingButton = wideButton("Manage shared items") { openSharing() }
        sharingCard.addView(manageSharingButton)
        sharingCard.addView(
            text(
                "Only the folders and files you choose are visible to the browser. Changes restart the server automatically and keep the same PIN.",
                12f,
                Color.rgb(107, 114, 128),
            ).apply { setPadding(0, dp(8), 0, 0) },
        )
        content.addView(sharingCard)

        credentialsCard = card().apply { visibility = View.GONE }
        credentialsCard.addView(text("Open this address on your computer", 13f, Color.rgb(75, 85, 99)))
        urlView = text("", 17f, Color.rgb(37, 99, 235), mono = true).apply {
            setTextIsSelectable(true)
            setPadding(0, dp(4), 0, dp(10))
        }
        credentialsCard.addView(urlView)
        hostnameView = text("", 13f, Color.rgb(107, 114, 128), mono = true).apply {
            setTextIsSelectable(true)
        }
        credentialsCard.addView(hostnameView)

        credentialsCard.addView(
            text("PIN", 13f, Color.rgb(75, 85, 99)).apply { setPadding(0, dp(14), 0, 0) },
        )
        pinView = text("", 34f, Color.rgb(17, 24, 39), bold = true, mono = true).apply {
            setTextIsSelectable(true)
            letterSpacing = 0.12f
        }
        credentialsCard.addView(pinView)

        val credentialsButtons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(0, dp(10), 0, dp(14))
        }
        copyUrlButton = button("Copy address").apply { setOnClickListener { copy(urlView.text.toString(), "Address") } }
        copyPinButton = button("Copy PIN").apply { setOnClickListener { copy(pinView.text.toString(), "PIN") } }
        credentialsButtons.addView(copyUrlButton, rowParams())
        credentialsButtons.addView(copyPinButton, rowParams())
        credentialsCard.addView(credentialsButtons)

        qrView = ImageView(this).apply {
            adjustViewBounds = true
            setBackgroundColor(Color.WHITE)
            setPadding(dp(8), dp(8), dp(8), dp(8))
        }
        val qrParams = LinearLayout.LayoutParams(dp(220), dp(220)).apply {
            gravity = Gravity.CENTER_HORIZONTAL
            bottomMargin = dp(12)
        }
        credentialsCard.addView(qrView, qrParams)
        openButton = button("Open in this phone's browser").apply {
            setOnClickListener { openUrl() }
        }
        credentialsCard.addView(openButton)
        content.addView(credentialsCard)

        val permissionCard = card()
        permissionCard.addView(text("Permissions", 18f, Color.rgb(17, 24, 39), bold = true))
        permissionCard.addView(
            text(
                "All files access is required to read the folders and files you share. The notification and battery opt-out keep transfers alive while the screen is off.",
                13f,
                Color.rgb(75, 85, 99),
            ).apply { setPadding(0, dp(4), 0, dp(10)) },
        )
        filesPermissionButton = wideButton("All files access") { openAllFilesSettings() }
        permissionCard.addView(filesPermissionButton)
        notificationPermissionButton = wideButton("Notification permission") { requestNotificationPermission() }
        permissionCard.addView(notificationPermissionButton)
        batteryButton = wideButton("Battery optimization settings") { openBatterySettings() }
        permissionCard.addView(batteryButton)
        content.addView(permissionCard)

        content.addView(
            text(
                "The server is available only on your local network and is protected by a PIN. Keep the phone connected to Wi-Fi.",
                12f,
                Color.rgb(107, 114, 128),
            ).apply { setPadding(dp(4), dp(16), dp(4), 0) },
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
                statusView.setTextColor(Color.rgb(37, 99, 235))
            }
            running -> {
                statusView.text = "Server running"
                statusView.setTextColor(Color.rgb(21, 128, 61))
            }
            else -> {
                statusView.text = "Server stopped"
                statusView.setTextColor(Color.rgb(17, 24, 39))
            }
        }
        errorView.visibility = if (!state?.error.isNullOrBlank()) View.VISIBLE else View.GONE
        errorView.text = state?.error.orEmpty()

        startButton.isEnabled = !running && !starting
        stopButton.isEnabled = running || starting
        copyUrlButton.isEnabled = !state?.url.isNullOrBlank()
        copyPinButton.isEnabled = !state?.pin.isNullOrBlank()
        openButton.isEnabled = !state?.url.isNullOrBlank()

        if (state?.pin != null || state?.url != null) {
            credentialsCard.visibility = View.VISIBLE
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
            credentialsCard.visibility = View.GONE
            lastQrText = null
        }

        val allFiles = Environment.isExternalStorageManager()
        filesPermissionButton.text = if (allFiles) "All files access: granted" else "Grant all files access"
        filesPermissionButton.isEnabled = !allFiles
        val notifications = Build.VERSION.SDK_INT < 33 ||
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        notificationPermissionButton.text =
            if (notifications) "Notifications: allowed" else "Allow notifications"
        notificationPermissionButton.isEnabled = !notifications
        updateSharingSummary()
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

    private fun text(
        value: String,
        size: Float,
        color: Int,
        bold: Boolean = false,
        mono: Boolean = false,
    ): TextView {
        return TextView(this).apply {
            text = value
            textSize = size
            setTextColor(color)
            if (bold) setTypeface(typeface, Typeface.BOLD)
            if (mono) typeface = Typeface.MONOSPACE
            setLineSpacing(0f, 1.05f)
        }
    }

    private fun card(): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), dp(16), dp(16), dp(16))
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(16).toFloat()
            }
            elevation = dp(1).toFloat()
            val params = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            )
            params.bottomMargin = dp(14)
            layoutParams = params
        }
    }

    private fun button(label: String): Button {
        return Button(this).apply {
            text = label
            textSize = 14f
            isAllCaps = false
            minHeight = dp(44)
            backgroundTintList = ColorStateList.valueOf(Color.rgb(229, 231, 235))
            setTextColor(Color.rgb(17, 24, 39))
        }
    }

    private fun wideButton(label: String, action: () -> Unit): Button {
        return button(label).apply {
            setOnClickListener { action() }
            val params = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            )
            params.topMargin = dp(6)
            layoutParams = params
        }
    }

    private fun rowParams(): LinearLayout.LayoutParams {
        return LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply {
            marginEnd = dp(6)
        }
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    private fun toast(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
    }
}
