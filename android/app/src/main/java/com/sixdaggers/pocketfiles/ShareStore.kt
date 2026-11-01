package com.sixdaggers.pocketfiles

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

object ShareStore {
    enum class Mode { SELECTED, EVERYTHING }
    enum class AddResult { ADDED, COVERED, INVALID }

    data class Share(
        val label: String,
        val uri: String,
        val path: String,
        val kind: String,
    )

    private const val PREFS = "troph_shares"
    private const val KEY_SHARES = "shares"
    private const val KEY_MODE = "mode"

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun mode(context: Context): Mode =
        if (prefs(context).getString(KEY_MODE, null) == "everything") {
            Mode.EVERYTHING
        } else {
            Mode.SELECTED
        }

    fun setMode(context: Context, mode: Mode) {
        prefs(context)
            .edit()
            .putString(KEY_MODE, if (mode == Mode.EVERYTHING) "everything" else "selected")
            .apply()
    }

    fun shares(context: Context): List<Share> {
        val raw = prefs(context).getString(KEY_SHARES, null) ?: return emptyList()
        return try {
            val array = JSONArray(raw)
            (0 until array.length()).mapNotNull { index ->
                val obj = array.optJSONObject(index) ?: return@mapNotNull null
                val path = obj.optString("path").ifBlank { return@mapNotNull null }
                Share(
                    label = obj.optString("label").ifBlank { File(path).name },
                    uri = obj.optString("uri"),
                    path = path,
                    kind = if (obj.optString("kind") == "file") "file" else "folder",
                )
            }
        } catch (_: Throwable) {
            emptyList()
        }
    }

    private fun save(context: Context, shares: List<Share>) {
        val array = JSONArray()
        for (share in shares) {
            array.put(
                JSONObject()
                    .put("label", share.label)
                    .put("uri", share.uri)
                    .put("path", share.path)
                    .put("kind", share.kind),
            )
        }
        prefs(context).edit().putString(KEY_SHARES, array.toString()).apply()
    }

    fun add(context: Context, share: Share): AddResult {
        val canonical = try {
            File(share.path).canonicalPath
        } catch (_: Throwable) {
            return AddResult.INVALID
        }
        val current = shares(context).toMutableList()

        for (existing in current) {
            if (existing.kind == "folder" &&
                (canonical == existing.path || canonical.startsWith(existing.path + File.separator))
            ) {
                return AddResult.COVERED
            }
        }

        if (share.kind == "folder") {
            current.removeAll { existing ->
                existing.path == canonical || existing.path.startsWith(canonical + File.separator)
            }
        } else {
            current.removeAll { existing -> existing.path == canonical }
        }

        current.add(share.copy(path = canonical))
        save(context, current)
        return AddResult.ADDED
    }

    fun remove(context: Context, path: String) {
        save(context, shares(context).filterNot { it.path == path })
    }

    fun clear(context: Context) {
        prefs(context).edit().remove(KEY_SHARES).apply()
    }
}
