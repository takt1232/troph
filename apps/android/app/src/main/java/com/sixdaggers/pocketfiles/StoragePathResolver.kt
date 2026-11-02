package com.sixdaggers.pocketfiles

import android.content.Context
import android.net.Uri
import android.os.Environment
import android.os.storage.StorageManager
import android.provider.DocumentsContract
import android.provider.MediaStore
import android.provider.OpenableColumns
import java.io.File

object StoragePathResolver {
    data class Resolved(
        val path: String,
        val label: String,
        val isDirectory: Boolean,
    )

    fun resolve(context: Context, uri: Uri, expectDirectory: Boolean): Resolved? {
        val raw = pathFor(context, uri) ?: return null
        val file = try {
            File(raw).canonicalFile
        } catch (_: Throwable) {
            return null
        }
        if (expectDirectory && !file.isDirectory) return null
        if (!expectDirectory && !file.isFile) return null
        if (!isAllowed(context, file)) return null
        return Resolved(
            path = file.absolutePath,
            label = labelFor(context, uri, file),
            isDirectory = file.isDirectory,
        )
    }

    fun isAllowed(context: Context, file: File): Boolean {
        if (!isInsideStorage(context, file)) return false
        val parts = file.path.split(File.separatorChar).filter { it.isNotEmpty() }
        for (index in 0 until parts.size - 1) {
            if (parts[index] == "Android" &&
                (parts[index + 1] == "data" || parts[index + 1] == "obb")
            ) {
                return false
            }
        }
        return true
    }

    private fun isInsideStorage(context: Context, file: File): Boolean {
        val roots = mutableListOf(Environment.getExternalStorageDirectory())
        val sm = context.getSystemService(StorageManager::class.java)
        for (volume in sm.storageVolumes) {
            volume.directory?.let { roots.add(it) }
        }
        val path = file.absolutePath
        return roots.any { root ->
            val rootPath = try {
                root.canonicalPath
            } catch (_: Throwable) {
                root.absolutePath
            }
            path == rootPath || path.startsWith(rootPath + File.separator)
        }
    }

    private fun pathFor(context: Context, uri: Uri): String? {
        if (uri.scheme == "file") return uri.path
        if (uri.scheme != "content") return null

        val authority = uri.authority ?: return null
        val documentId = try {
            if (DocumentsContract.isTreeUri(uri)) {
                DocumentsContract.getTreeDocumentId(uri)
            } else {
                DocumentsContract.getDocumentId(uri)
            }
        } catch (_: Throwable) {
            null
        } ?: return null

        return when (authority) {
            "com.android.externalstorage.documents" -> {
                val parts = documentId.split(":", limit = 2)
                val volume = parts[0]
                val relative = parts.getOrElse(1) { "" }.trimStart('/')
                val base = if (volume.equals("primary", ignoreCase = true)) {
                    Environment.getExternalStorageDirectory()
                } else {
                    File("/storage/$volume")
                }
                File(base, relative).path
            }

            "com.android.providers.downloads.documents" -> when {
                documentId.startsWith("raw:") -> documentId.removePrefix("raw:")
                documentId.startsWith("msf:") -> {
                    val id = documentId.removePrefix("msf:")
                    mediaPath(context, MediaStore.Downloads.EXTERNAL_CONTENT_URI, id)
                        ?: mediaPath(context, MediaStore.Files.getContentUri("external"), id)
                }
                else -> null
            }

            "com.android.providers.media.documents" -> {
                val id = documentId.substringAfter(':', documentId)
                mediaPath(context, MediaStore.Files.getContentUri("external"), id)
            }

            else -> if (documentId.startsWith("/")) documentId else null
        }
    }

    private fun mediaPath(context: Context, collection: Uri, id: String): String? {
        return try {
            context.contentResolver.query(
                collection,
                arrayOf(MediaStore.MediaColumns.DATA),
                "_id = ?",
                arrayOf(id),
                null,
            )?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        } catch (_: Throwable) {
            null
        }
    }

    private fun labelFor(context: Context, uri: Uri, file: File): String {
        if (file.name.isNotBlank()) return file.name
        return try {
            context.contentResolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME),
                null,
                null,
                null,
            )?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        } catch (_: Throwable) {
            null
        } ?: "Shared item"
    }
}
