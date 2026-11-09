package dev.puente.app

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import android.view.Surface

/** Codificador H.264 que entrega cada paquete a Rust. */
internal class Encoder(width: Int, height: Int) {
    private val codec = MediaCodec.createEncoderByType(MIME_TYPE)
    private val thread = Thread(::drain, "puente-encoder")
    private var lastKeyRequest = 0L

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
            setInteger(MediaFormat.KEY_BIT_RATE, BIT_RATE)
            setInteger(MediaFormat.KEY_FRAME_RATE, FRAME_RATE)
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
            }
        } catch (error: IllegalStateException) {
            Log.w(TAG, "el codificador se detuvo", error)
        }
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
        const val BIT_RATE = 8_000_000
        const val FRAME_RATE = 60
        const val KEYFRAME_INTERVAL_SECONDS = 10
        const val REPEAT_AFTER_MICROSECONDS = 100_000L
        const val DEQUEUE_TIMEOUT_MICROSECONDS = 100_000L
        const val KEY_REQUEST_INTERVAL_MILLISECONDS = 500L
    }
}
