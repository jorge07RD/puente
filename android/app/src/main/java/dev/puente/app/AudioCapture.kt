package dev.puente.app

import android.annotation.SuppressLint
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.projection.MediaProjection
import java.nio.ByteBuffer
import kotlin.concurrent.thread

/** Captura lo que suena en el móvil y lo entrega a Rust. Quien la crea comprueba el permiso. */
@SuppressLint("MissingPermission")
internal class AudioCapture(projection: MediaProjection) {
    private val record: AudioRecord

    @Volatile
    private var running = true

    private val reader: Thread

    init {
        val capture = AudioPlaybackCaptureConfiguration.Builder(projection)
            .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
            .addMatchingUsage(AudioAttributes.USAGE_GAME)
            .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            .build()
        val format = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
            .setSampleRate(SAMPLE_RATE)
            .setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
            .build()
        record = AudioRecord.Builder()
            .setAudioFormat(format)
            .setBufferSizeInBytes(PACKET_BYTES * 8)
            .setAudioPlaybackCaptureConfig(capture)
            .build()
        record.startRecording()
        reader = thread(name = "puente-audio") { pump() }
    }

    private fun pump() {
        val buffer = ByteBuffer.allocateDirect(PACKET_BYTES)
        while (running) {
            val read = record.read(buffer, PACKET_BYTES)
            if (read > 0) Native.onAudio(buffer, read)
        }
    }

    fun stop() {
        running = false
        record.stop()
        reader.join()
        record.release()
    }

    private companion object {
        const val SAMPLE_RATE = 48_000

        /** 20 ms de audio estéreo de 16 bits. */
        const val PACKET_BYTES = SAMPLE_RATE / 50 * 2 * 2
    }
}
