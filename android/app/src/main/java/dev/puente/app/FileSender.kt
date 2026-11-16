package dev.puente.app

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import java.io.IOException

/** Envía a la PC un archivo de Android, venga de "Compartir" o del selector de la app. */
internal object FileSender {
    private const val TAG = "puente"
    private const val CHUNK_BYTES = 64 * 1024

    /** Bloquea mientras envía: hay que llamarlo desde un hilo propio. */
    fun send(resolver: ContentResolver, uri: Uri): Boolean {
        val (name, size) = describe(resolver, uri) ?: return false
        val id = Native.beginFile(name, size)
        if (id < 0) return false

        val success = try {
            pump(resolver, uri, id)
        } catch (error: IOException) {
            Log.w(TAG, "no se pudo leer el archivo", error)
            false
        }
        Native.endFile(id, success)
        return success
    }

    private fun pump(resolver: ContentResolver, uri: Uri, id: Int): Boolean {
        val input = resolver.openInputStream(uri) ?: return false
        input.use { stream ->
            val buffer = ByteArray(CHUNK_BYTES)
            while (true) {
                val read = stream.read(buffer)
                if (read < 0) return true
                if (read > 0 && !Native.fileChunk(id, buffer.copyOf(read))) return false
            }
        }
    }

    private fun describe(resolver: ContentResolver, uri: Uri): Pair<String, Long>? {
        resolver.query(uri, null, null, null, null)?.use { cursor ->
            val nameColumn = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            val sizeColumn = cursor.getColumnIndex(OpenableColumns.SIZE)
            if (cursor.moveToFirst() && nameColumn >= 0 && sizeColumn >= 0 && !cursor.isNull(sizeColumn)) {
                return cursor.getString(nameColumn) to cursor.getLong(sizeColumn)
            }
        }
        return null
    }
}
