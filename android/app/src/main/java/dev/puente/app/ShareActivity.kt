package dev.puente.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.widget.TextView
import android.widget.Toast
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
            val allSent = files.map { FileSender.send(contentResolver, it) }.all { it }
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

    private companion object {
        const val PADDING = 64
    }
}
