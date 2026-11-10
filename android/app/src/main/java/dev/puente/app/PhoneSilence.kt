package dev.puente.app

import android.content.Context
import android.media.AudioManager

/** Calla la música del móvil mientras suena en la PC. Llamadas y alarmas no se tocan. */
internal object PhoneSilence {
    private const val PREFERENCES = "silencio"
    private const val MUTED_BY_PUENTE = "silenciado"

    private var wanted = false
    private var capturing = false
    private var muted = false

    fun setWanted(context: Context, on: Boolean) {
        wanted = on
        apply(context)
    }

    fun setCapturing(context: Context, on: Boolean) {
        capturing = on
        apply(context)
    }

    /** Si la app murió con el móvil callado, le devuelve el sonido. */
    fun recover(context: Context) {
        val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        if (!preferences.getBoolean(MUTED_BY_PUENTE, false)) return
        adjust(context, AudioManager.ADJUST_UNMUTE)
        remember(context, false)
    }

    private fun apply(context: Context) {
        val shouldMute = wanted && capturing
        if (shouldMute == muted) return
        if (shouldMute) {
            // Si ya estaba en silencio, es decisión de quien lo usa: al terminar debe seguir así.
            if (context.getSystemService(AudioManager::class.java).isStreamMute(AudioManager.STREAM_MUSIC)) return
            remember(context, true)
            adjust(context, AudioManager.ADJUST_MUTE)
        } else {
            adjust(context, AudioManager.ADJUST_UNMUTE)
            remember(context, false)
        }
        muted = shouldMute
    }

    private fun adjust(context: Context, direction: Int) {
        context.getSystemService(AudioManager::class.java)
            .adjustStreamVolume(AudioManager.STREAM_MUSIC, direction, 0)
    }

    /** Se escribe al momento: el proceso puede morir justo después. */
    private fun remember(context: Context, value: Boolean) {
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(MUTED_BY_PUENTE, value)
            .commit()
    }
}
