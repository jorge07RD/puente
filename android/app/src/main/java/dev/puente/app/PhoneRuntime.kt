package dev.puente.app

import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.Ringtone
import android.media.RingtoneManager
import android.os.BatteryManager
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.util.Log
import android.webkit.MimeTypeMap
import org.json.JSONException
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.util.Locale
import kotlin.concurrent.thread

/** Lo que debe existir una sola vez por proceso, lo arranque quien lo arranque. */
internal object PhoneRuntime {
    private const val TAG = "puente"
    private const val WAIT_MILLISECONDS = 1000
    private const val RING_MILLISECONDS = 30_000L

    private val handler = Handler(Looper.getMainLooper())
    private var started = false
    private var ringtone: Ringtone? = null

    /** El último texto compartido, para no devolverlo al lado que lo envió. */
    var lastClipboard: String? = null
        private set

    fun ensureStarted(context: Context) {
        synchronized(this) {
            if (started) return
            started = true
        }
        val app = context.applicationContext
        PhoneSilence.recover(app)
        Native.setSystemLanguage(Locale.getDefault().toLanguageTag())
        Native.ensureEngine(app.filesDir.path)
        watchBattery(app)
        thread(name = "puente-tareas", isDaemon = true) {
            while (true) {
                val task = Native.waitTask(WAIT_MILLISECONDS) ?: continue
                run(app, task)
            }
        }
    }

    /** Envía el portapapeles a la PC si cambió desde la última vez. */
    fun shareClipboard(context: Context) {
        val manager = context.getSystemService(ClipboardManager::class.java)
        val text = manager.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString() ?: return
        if (text.isEmpty() || text == lastClipboard) return
        lastClipboard = text
        Native.onClipboard(text)
    }

    private fun watchBattery(app: Context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                val level = intent.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
                val scale = intent.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
                if (level < 0 || scale <= 0) return
                val status = intent.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
                val charging = status == BatteryManager.BATTERY_STATUS_CHARGING ||
                    status == BatteryManager.BATTERY_STATUS_FULL
                Native.onBattery(level * 100 / scale, charging)
            }
        }
        app.registerReceiver(receiver, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
    }

    private fun run(app: Context, text: String) {
        try {
            val task = JSONObject(text)
            when (task.getString("command")) {
                "dismiss" -> {
                    val key = task.getString("key")
                    handler.post { NotifService.instance?.cancelNotification(key) }
                }
                "reply" -> {
                    val key = task.getString("key")
                    val value = task.getString("text")
                    handler.post {
                        NotifService.instance?.reply(key, value) ?: Native.onReplyResult(key, false)
                    }
                }
                "call_action" -> {
                    val key = task.getString("key")
                    val answer = task.getBoolean("answer")
                    handler.post { NotifService.instance?.handleCall(key, answer) }
                }
                "ring" -> {
                    val on = task.getBoolean("on")
                    handler.post { setRinging(app, on) }
                }
                "media" -> {
                    val action = task.getString("action")
                    handler.post { NotifService.instance?.controlMedia(action) }
                }
                "set_clipboard" -> {
                    val value = task.getString("text")
                    handler.post { setClipboard(app, value) }
                }
                "mute_phone" -> {
                    val on = task.getBoolean("on")
                    handler.post { PhoneSilence.setWanted(app, on) }
                }
                "quality" -> {
                    val level = task.getString("level")
                    handler.post {
                        if (CaptureQuality.set(level)) MirrorService.instance?.restartEncoderIfNeeded()
                    }
                }
                "stop_mirror" -> app.stopService(Intent(app, MirrorService::class.java))
                "save_file" -> saveToDownloads(app, File(task.getString("path")), task.getString("name"))
            }
        } catch (error: JSONException) {
            Log.w(TAG, "tarea no válida", error)
        } catch (error: IOException) {
            Log.w(TAG, "no se pudo guardar el archivo", error)
        }
    }

    private fun setClipboard(app: Context, text: String) {
        lastClipboard = text
        app.getSystemService(ClipboardManager::class.java)
            .setPrimaryClip(ClipData.newPlainText("Puente", text))
    }

    private fun setRinging(app: Context, on: Boolean) {
        ringtone?.stop()
        ringtone = null
        if (!on) return

        val uri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM)
        val started = RingtoneManager.getRingtone(app, uri) ?: return
        started.audioAttributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).build()
        started.isLooping = true
        started.play()
        ringtone = started
        handler.postDelayed({ if (ringtone === started) setRinging(app, false) }, RING_MILLISECONDS)
    }

    private fun saveToDownloads(app: Context, source: File, name: String) {
        val resolver = app.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, name)
            put(MediaStore.Downloads.IS_PENDING, 1)
            MimeTypeMap.getSingleton()
                .getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase())
                ?.let { put(MediaStore.Downloads.MIME_TYPE, it) }
        }
        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: throw IOException("Android no creó el archivo")
        resolver.openOutputStream(uri)?.use { output ->
            source.inputStream().use { it.copyTo(output) }
        } ?: throw IOException("no se pudo abrir el destino")

        resolver.update(uri, ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }, null, null)
        source.delete()
    }
}
