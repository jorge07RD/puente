package dev.puente.app

import java.nio.ByteBuffer

object Native {
    init {
        System.loadLibrary("puente_android")
    }

    external fun ensureEngine(dataDir: String)

    /** Etiqueta del idioma del sistema, como `es-DO`. */
    external fun setSystemLanguage(tag: String)

    /** Código del idioma de la interfaz: `es`, `en` o `pt`. */
    external fun language(): String

    /** Deja una línea en el registro que guarda la PC. */
    external fun log(text: String)

    external fun onDisplay(top: Int, bottom: Int, fontScale: Float)

    external fun onBatteryUnrestricted(unrestricted: Boolean)

    /** Verdadero mientras hay una PC conectada. */
    external fun isLinked(): Boolean

    /** Devuelve true si el codificador debe generar un fotograma clave. */
    external fun onVideoFrame(buffer: ByteBuffer, offset: Int, size: Int, flags: Int): Boolean

    external fun onMirrorState(active: Boolean)

    external fun onAudio(buffer: ByteBuffer, size: Int)

    external fun onInputState(enabled: Boolean)

    /** Espera la siguiente orden de la PC, en JSON, o devuelve null al agotar el tiempo. */
    external fun waitInput(timeoutMilliseconds: Int): String?

    /** Igual que [waitInput], para las tareas que no dependen de compartir pantalla. */
    external fun waitTask(timeoutMilliseconds: Int): String?

    external fun onNotificationAccess(enabled: Boolean)

    external fun onNotification(key: String, app: String, title: String, body: String, canReply: Boolean)

    external fun onCall(key: String, app: String, caller: String, canAnswer: Boolean, canDecline: Boolean)

    external fun onCallEnded(key: String)

    external fun onReplyResult(key: String, delivered: Boolean)

    external fun onNotificationRemoved(key: String)

    external fun onBattery(percent: Int, charging: Boolean)

    /** Un título nulo significa que no hay nada reproduciéndose. */
    external fun onMedia(title: String?, artist: String?, playing: Boolean)

    external fun onClipboard(text: String)

    /** Devuelve el identificador del archivo, o -1 si no hay conexión. */
    external fun beginFile(name: String, size: Long): Int

    external fun fileChunk(id: Int, data: ByteArray): Boolean

    external fun endFile(id: Int, success: Boolean)
}
