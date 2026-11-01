package com.sixdaggers.pocketfiles

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import java.io.File

class SharingActivity : Activity() {
    companion object {
        private const val REQ_TREE = 100
        private const val REQ_FILES = 101
    }

    private var rendering = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setDecorFitsSystemWindows(false)
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT
        render()
        if (Build.VERSION.SDK_INT >= 30) {
            window.insetsController?.setSystemBarsAppearance(
                android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                    android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
                android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                    android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
            )
        }
    }

    private fun render() {
        setContentView(buildContent())
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

        content.addView(text("Shared items", 30f, Color.rgb(17, 24, 39), bold = true))
        content.addView(
            text(
                "Choose what Troph makes available to the browser. Nothing is shared by default.",
                15f,
                Color.rgb(75, 85, 99),
            ).apply { setPadding(0, dp(4), 0, dp(18)) },
        )

        val mode = ShareStore.mode(this)
        val modeCard = card()
        val toggle = Switch(this).apply {
            text = "Share everything"
            textSize = 16f
            setTextColor(Color.rgb(17, 24, 39))
            isChecked = mode == ShareStore.Mode.EVERYTHING
        }
        toggle.setOnCheckedChangeListener { _, checked ->
            if (rendering) return@setOnCheckedChangeListener
            val target = if (checked) {
                ShareStore.Mode.EVERYTHING
            } else {
                ShareStore.Mode.SELECTED
            }
            if (ShareStore.mode(this) != target) {
                ShareStore.setMode(this, target)
                afterSharingChange(
                    "The sharing mode was changed.",
                    "Sharing mode updated.",
                )
            }
        }
        modeCard.addView(toggle)
        modeCard.addView(
            text(
                "Shares internal storage and every SD or USB volume. Android/data and Android/obb are never exposed.",
                13f,
                Color.rgb(75, 85, 99),
            ).apply { setPadding(0, dp(6), 0, 0) },
        )
        content.addView(modeCard)

        val itemsCard = card()
        itemsCard.addView(text("Selected items", 18f, Color.rgb(17, 24, 39), bold = true))
        if (mode == ShareStore.Mode.EVERYTHING) {
            itemsCard.addView(
                text(
                    "All storage is shared. Turn off Share everything to choose individual folders and files.",
                    14f,
                    Color.rgb(75, 85, 99),
                ).apply { setPadding(0, dp(8), 0, 0) },
            )
        } else {
            val shares = ShareStore.shares(this)
            if (shares.isEmpty()) {
                itemsCard.addView(
                    text(
                        "Nothing is shared yet. The server can still run, but the browser will show an empty library.",
                        14f,
                        Color.rgb(75, 85, 99),
                    ).apply { setPadding(0, dp(8), 0, dp(8)) },
                )
            } else {
                for (share in shares) {
                    itemsCard.addView(shareRow(share))
                }
            }

            val buttons = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(0, dp(10), 0, 0)
            }
            buttons.addView(
                button("Add folder").apply { setOnClickListener { pickFolder() } },
                rowParams(),
            )
            buttons.addView(
                button("Add files").apply { setOnClickListener { pickFiles() } },
                rowParams(),
            )
            itemsCard.addView(buttons)

            if (shares.isNotEmpty()) {
                itemsCard.addView(
                    wideButton("Remove all") {
                        ShareStore.clear(this)
                        afterSharingChange(
                            "All shared items were removed.",
                            "All items removed.",
                        )
                    },
                )
            }
        }
        content.addView(itemsCard)

        content.addView(
            text(
                "Changes restart the server automatically and keep the same PIN and browser session.",
                12f,
                Color.rgb(107, 114, 128),
            ).apply { setPadding(dp(4), dp(6), dp(4), 0) },
        )

        return root
    }

    private fun shareRow(share: ShareStore.Share): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(10), 0, dp(10))
        }
        val info = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                1f,
            )
        }
        info.addView(text(share.label, 15f, Color.rgb(17, 24, 39), bold = true))
        info.addView(
            text(
                if (share.kind == "file") "File" else "Folder",
                12f,
                Color.rgb(75, 85, 99),
            ),
        )
        if (!File(share.path).exists()) {
            info.addView(
                text(
                    "Missing \u2014 remove and choose again",
                    12f,
                    Color.rgb(185, 28, 28),
                ),
            )
        }
        info.addView(
            text(share.path, 11f, Color.rgb(107, 114, 128), mono = true).apply {
                setTextIsSelectable(true)
            },
        )
        row.addView(info)
        row.addView(
            button("Remove").apply {
                setOnClickListener {
                    ShareStore.remove(this@SharingActivity, share.path)
                    afterSharingChange("A shared item was removed.", "Removed.")
                }
            },
        )
        return row
    }

    private fun pickFolder() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
        }
        startActivityForResult(intent, REQ_TREE)
    }

    private fun pickFiles() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
        }
        startActivityForResult(intent, REQ_FILES)
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (resultCode != RESULT_OK || data == null) return
        if (requestCode != REQ_TREE && requestCode != REQ_FILES) return

        val uris = mutableListOf<Uri>()
        val clip = data.clipData
        if (clip != null) {
            for (index in 0 until clip.itemCount) {
                uris.add(clip.getItemAt(index).uri)
            }
        } else {
            data.data?.let { uris.add(it) }
        }

        var added = 0
        var covered = 0
        var skipped = 0
        for (uri in uris) {
            try {
                contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION,
                )
            } catch (_: Throwable) {
            }
            val resolved = StoragePathResolver.resolve(
                this,
                uri,
                expectDirectory = requestCode == REQ_TREE,
            )
            if (resolved == null) {
                skipped += 1
                continue
            }
            val share = ShareStore.Share(
                label = resolved.label,
                uri = uri.toString(),
                path = resolved.path,
                kind = if (resolved.isDirectory) "folder" else "file",
            )
            when (ShareStore.add(this, share)) {
                ShareStore.AddResult.ADDED -> added += 1
                ShareStore.AddResult.COVERED -> covered += 1
                ShareStore.AddResult.INVALID -> skipped += 1
            }
        }

        val messages = mutableListOf<String>()
        if (added > 0) {
            messages.add(if (added == 1) "1 item added." else "$added items added.")
        }
        if (covered > 0) {
            messages.add(
                if (covered == 1) {
                    "1 item is already covered by a shared folder."
                } else {
                    "$covered items are already covered by a shared folder."
                },
            )
        }
        if (skipped > 0) {
            messages.add(
                if (skipped == 1) {
                    "1 item couldn't be shared. Choose a folder or file on this device."
                } else {
                    "$skipped items couldn't be shared. Choose folders or files on this device."
                },
            )
        }
        val summary = messages.joinToString(" ")
        if (added > 0) {
            val change = if (summary.isBlank()) {
                "New shared items were added."
            } else {
                "$summary New shared items were added."
            }
            afterSharingChange(change, summary)
        } else {
            if (summary.isNotBlank()) toast(summary)
            render()
        }
    }

    private fun serverIsActive(): Boolean {
        val state = ServerService.state
        return state?.running == true || state?.starting == true
    }

    private fun afterSharingChange(changeMessage: String, toastMessage: String) {
        if (!serverIsActive()) {
            if (toastMessage.isNotBlank()) toast(toastMessage)
            render()
            return
        }
        restartServer()
        AlertDialog.Builder(this)
            .setTitle("Restart required")
            .setMessage(
                "$changeMessage The server was running, so Troph is restarting it automatically " +
                    "to apply the change. It keeps the same PIN and browser session.",
            )
            .setPositiveButton("OK") { _, _ -> render() }
            .setCancelable(false)
            .show()
    }

    private fun restartServer() {
        startService(
            Intent(this, ServerService::class.java).setAction(ServerService.ACTION_RESTART),
        )
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
