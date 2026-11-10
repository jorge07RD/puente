package dev.puente.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.widget.TextView
import android.widget.Toast
import java.io.IOException
import kotlin.concurrent.thread

/** Destino de "Compartir": envía a la PC archivos o texto. */
class ShareActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        PhoneRuntime.ensureStarted(this)
        setContentView(TextView(this).apply {
            text = PhoneTexts.sending
            setPadding(PADDING, PADDING, PADDING, PADDING)
        })

        val files = sharedFiles()
        if (files.isEmpty()) {
            intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.let { Native.onClipboard(it.toString()) }
            finishWith(PhoneTexts.sent)
            return
        }

        // Android retira el acceso a estos archivos cuando esta pantalla se cierra.
        thread(name = "puente-compartir") {
            val allSent = files.map(::send).all { it }
            runOnUiThread { finishWith(if (allSent) PhoneTexts.sent else PhoneTexts.sendFailed) }
        }
    }

    private fun finishWith(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
        finish()
    }

    @Suppress("DEPRECATION")
    private fun sharedFiles(): List<Uri> {
        val single: Uri?
        val multiple: List<Uri>?
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            single = intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            multiple = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            single = intent.getParcelableExtra(Intent.EXTRA_STREAM)
            multiple = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)
        }
        return when (intent.action) {
            Intent.ACTION_SEND -> listOfNotNull(single)
            Intent.ACTION_SEND_MULTIPLE -> multiple.orEmpty()
            else -> emptyList()
        }
    }

    private fun send(uri: Uri): Boolean {
        val (name, size) = describe(uri) ?: return false
        val id = Native.beginFile(name, size)
        if (id < 0) return false

        val success = try {
            pump(uri, id)
        } catch (error: IOException) {
            Log.w(TAG, "no se pudo leer el archivo", error)
            false
        }
        Native.endFile(id, success)
        return success
    }

    private fun pump(uri: Uri, id: Int): Boolean {
        val input = contentResolver.openInputStream(uri) ?: return false
        input.use { stream ->
            val buffer = ByteArray(CHUNK_BYTES)
            while (true) {
                val read = stream.read(buffer)
                if (read < 0) return true
                if (read > 0 && !Native.fileChunk(id, buffer.copyOf(read))) return false
            }
        }
    }

    private fun describe(uri: Uri): Pair<String, Long>? {
        contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            val nameColumn = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            val sizeColumn = cursor.getColumnIndex(OpenableColumns.SIZE)
            if (cursor.moveToFirst() && nameColumn >= 0 && sizeColumn >= 0 && !cursor.isNull(sizeColumn)) {
                return cursor.getString(nameColumn) to cursor.getLong(sizeColumn)
            }
        }
        return null
    }

    private companion object {
        const val TAG = "puente"
        const val PADDING = 64
        const val CHUNK_BYTES = 64 * 1024
    }
}
