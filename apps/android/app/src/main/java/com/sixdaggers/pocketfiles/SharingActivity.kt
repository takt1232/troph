package com.sixdaggers.pocketfiles

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.res.ColorStateList
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.Toast
import java.io.File

class SharingActivity : Activity() {
    companion object {
        private const val REQ_TREE = 100
        private const val REQ_FILES = 101
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setDecorFitsSystemWindows(false)
        render()
    }

    private fun render() {
        setContentView(buildContent())
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

        content.addView(label("Shared items", 28f, TrophTheme.TEXT, bold = true))
        content.addView(
            label(
                "Choose what Troph makes available to the browser. Nothing is shared by default.",
                14f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(4), 0, dp(20)) },
        )

        val mode = ShareStore.mode(this)
        val modePanel = panel()
        val modeRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        modeRow.addView(
            label("Share everything", 16f, TrophTheme.TEXT),
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f),
        )
        val toggle = Switch(this).apply {
            isChecked = mode == ShareStore.Mode.EVERYTHING
            thumbTintList = ColorStateList(
                arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                intArrayOf(TrophTheme.ACCENT, TrophTheme.MUTED),
            )
            trackTintList = ColorStateList(
                arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                intArrayOf(0x664ADE80, TrophTheme.BORDER_STRONG),
            )
        }
        toggle.setOnCheckedChangeListener { _, checked ->
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
        modeRow.addView(toggle)
        modePanel.addView(modeRow)
        modePanel.addView(
            label(
                "Shares internal storage and every SD or USB volume. Android/data and Android/obb are never exposed.",
                13f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(8), 0, 0) },
        )
        content.addView(modePanel)

        content.addView(
            sectionTitle("Selected items"),
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(26) },
        )

        val itemsPanel = panel()
        itemsPanel.setPadding(dp(18), dp(4), dp(18), dp(4))
        val shares = if (mode == ShareStore.Mode.EVERYTHING) {
            emptyList()
        } else {
            ShareStore.shares(this)
        }
        if (mode == ShareStore.Mode.EVERYTHING) {
            itemsPanel.addView(
                label(
                    "All storage is shared. Turn off Share everything to choose individual folders and files.",
                    14f,
                    TrophTheme.MUTED,
                ).apply { setPadding(0, dp(14), 0, dp(14)) },
            )
        } else if (shares.isEmpty()) {
            itemsPanel.addView(
                label(
                    "Nothing is shared yet. The server can still run, but the browser will show an empty library.",
                    14f,
                    TrophTheme.MUTED,
                ).apply { setPadding(0, dp(14), 0, dp(14)) },
            )
        } else {
            for (index in shares.indices) {
                itemsPanel.addView(shareRow(shares[index]))
                if (index < shares.lastIndex) itemsPanel.addView(rule())
            }
        }
        content.addView(
            itemsPanel,
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(10) },
        )

        if (mode == ShareStore.Mode.SELECTED) {
            val buttons = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(0, dp(14), 0, 0)
            }
            buttons.addView(secondaryButton("Add folder") { pickFolder() }, rowParams())
            buttons.addView(secondaryButton("Add files") { pickFiles() }, rowParams().apply { marginEnd = 0 })
            content.addView(buttons)

            if (shares.isNotEmpty()) {
                content.addView(
                    dangerButton("Remove all") {
                        ShareStore.clear(this)
                        afterSharingChange(
                            "All shared items were removed.",
                            "All items removed.",
                        )
                    },
                    LinearLayout.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                    ).apply { topMargin = dp(8) },
                )
            }
        }

        content.addView(
            label(
                "Changes restart the server automatically and keep the same PIN and browser session.",
                12f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(18), 0, 0) },
        )

        return root
    }

    private fun shareRow(share: ShareStore.Share): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(12), 0, dp(12))
        }
        val info = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                1f,
            )
        }
        info.addView(label(share.label, 15f, TrophTheme.TEXT, bold = true))
        info.addView(
            label(
                if (share.kind == "file") "File" else "Folder",
                12f,
                TrophTheme.MUTED,
            ).apply { setPadding(0, dp(2), 0, 0) },
        )
        if (!File(share.path).exists()) {
            info.addView(
                label(
                    "Missing \u2014 remove and choose again",
                    12f,
                    TrophTheme.DANGER,
                ).apply { setPadding(0, dp(2), 0, 0) },
            )
        }
        info.addView(
            label(share.path, 11f, TrophTheme.MUTED, mono = true).apply {
                setTextIsSelectable(true)
                setPadding(0, dp(4), 0, 0)
            },
        )
        row.addView(info)
        row.addView(
            smallButton("Remove") {
                ShareStore.remove(this@SharingActivity, share.path)
                afterSharingChange("A shared item was removed.", "Removed.")
            },
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ).apply { marginStart = dp(10) },
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

    private fun toast(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
    }
}
