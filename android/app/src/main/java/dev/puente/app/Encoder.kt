package dev.puente.app

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import android.view.Surface
import java.util.Locale
import kotlin.math.max
import kotlin.math.min

/** Codificador H.264 que entrega cada paquete a Rust. */
internal class Encoder(width: Int, height: Int, private val quality: CaptureQuality.Level) {
    private val codec = MediaCodec.createEncoderByType(MIME_TYPE)
    private val thread = Thread(::drain, "puente-encoder")
    private var lastKeyRequest = 0L

    /** Modo automático: el bitrate que se está usando y cuándo se tocó o se atascó la red. */
    private var bitRate = if (quality.adaptive) CaptureQuality.autoBitRate else quality.bitRate
    private var dropping = false
    private var lastCongestion = 0L
    private var lastBitRateChange = 0L

    @Volatile
    private var running = true

    val surface: Surface

    init {
        codec.configure(format(width, height), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        surface = codec.createInputSurface()
        codec.start()
        thread.start()
    }

    private fun format(width: Int, height: Int) =
        MediaFormat.createVideoFormat(MIME_TYPE, width, height).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline)
            setInteger(MediaFormat.KEY_BIT_RATE, bitRate)
            setInteger(MediaFormat.KEY_FRAME_RATE, quality.frameRate)
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, KEYFRAME_INTERVAL_SECONDS)
            setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, REPEAT_AFTER_MICROSECONDS)
            setInteger(MediaFormat.KEY_PRIORITY, 0)
            setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
        }

    private fun drain() {
        val info = MediaCodec.BufferInfo()
        try {
            while (running) {
                val index = codec.dequeueOutputBuffer(info, DEQUEUE_TIMEOUT_MICROSECONDS)
                if (index < 0) continue
                val buffer = codec.getOutputBuffer(index)
                val needsKeyframe = buffer != null && info.size > 0 &&
                    Native.onVideoFrame(buffer, info.offset, info.size, info.flags)
                codec.releaseOutputBuffer(index, false)
                if (needsKeyframe) requestKeyframe()
                if (quality.adaptive) adapt(needsKeyframe)
            }
        } catch (error: IllegalStateException) {
            Log.w(TAG, "el codificador se detuvo", error)
        }
    }

    /**
     * Rust tira cuadros cuando la red no da abasto. El primero de cada racha es un atasco:
     * se baja el bitrate enseguida. Tras un rato sin atascos se sube poco a poco.
     * Cambiarlo no reinicia el codificador, así que la imagen no se corta.
     */
    private fun adapt(dropped: Boolean) {
        val now = SystemClock.elapsedRealtime()
        val congested = dropped && !dropping
        dropping = dropped
        val target = when {
            congested && now - lastBitRateChange >= LOWER_INTERVAL_MILLISECONDS -> {
                lastCongestion = now
                max(MIN_AUTO_BIT_RATE, (bitRate * LOWER_FACTOR).toInt())
            }
            congested -> {
                lastCongestion = now
                return
            }
            now - lastCongestion >= CALM_MILLISECONDS &&
                now - lastBitRateChange >= RAISE_INTERVAL_MILLISECONDS ->
                min(MAX_AUTO_BIT_RATE, (bitRate * RAISE_FACTOR).toInt())
            else -> return
        }
        if (target == bitRate) return
        bitRate = target
        lastBitRateChange = now
        CaptureQuality.autoBitRate = target
        codec.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, target) })
        Native.log("calidad auto: %.1f Mbps".format(Locale.ROOT, target / 1_000_000.0))
    }

    private fun requestKeyframe() {
        val now = SystemClock.elapsedRealtime()
        if (now - lastKeyRequest < KEY_REQUEST_INTERVAL_MILLISECONDS) return
        lastKeyRequest = now
        codec.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0) })
    }

    fun stop() {
        running = false
        thread.join()
        codec.stop()
        codec.release()
        surface.release()
    }

    private companion object {
        const val TAG = "puente"
        const val MIME_TYPE = "video/avc"
        const val KEYFRAME_INTERVAL_SECONDS = 10
        const val REPEAT_AFTER_MICROSECONDS = 100_000L
        const val DEQUEUE_TIMEOUT_MICROSECONDS = 100_000L
        const val KEY_REQUEST_INTERVAL_MILLISECONDS = 500L
        const val MIN_AUTO_BIT_RATE = 2_000_000
        const val MAX_AUTO_BIT_RATE = 16_000_000
        const val LOWER_FACTOR = 0.6
        const val RAISE_FACTOR = 1.15
        /** Entre dos bajadas, para que la primera tenga tiempo de notarse. */
        const val LOWER_INTERVAL_MILLISECONDS = 1_000L
        /** Sin atascos durante este tiempo se empieza a subir. */
        const val CALM_MILLISECONDS = 5_000L
        const val RAISE_INTERVAL_MILLISECONDS = 2_000L
    }
}
