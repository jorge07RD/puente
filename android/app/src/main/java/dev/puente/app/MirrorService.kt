package dev.puente.app

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.content.res.Configuration
import android.graphics.drawable.Icon
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.Process
import android.util.DisplayMetrics
import android.util.Log
import android.view.Display
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** Mantiene viva la captura de pantalla mientras se comparte. */
class MirrorService : Service() {
    private data class CaptureSize(val width: Int, val height: Int, val dpi: Int)

    private val handler = Handler(Looper.getMainLooper())
    private var projection: MediaProjection? = null
    private var display: VirtualDisplay? = null
    private var encoder: Encoder? = null
    private var audio: AudioCapture? = null
    private var size: CaptureSize? = null
    private var screenLock: PowerManager.WakeLock? = null

    /** La pantalla sigue encendida solo mientras alguien la está mirando desde la PC. */
    private val screenKeeper = object : Runnable {
        override fun run() {
            val lock = screenLock ?: return
            when {
                Native.isLinked() && !lock.isHeld -> lock.acquire()
                !Native.isLinked() && lock.isHeld -> lock.release()
            }
            handler.postDelayed(this, SCREEN_CHECK_MILLISECONDS)
        }
    }

    private val projectionCallback = object : MediaProjection.Callback() {
        override fun onStop() {
            stopSelf()
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent == null || intent.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        startForeground(NOTIFICATION_ID, buildNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
        if (projection == null) {
            try {
                startCapture(intent)
            } catch (error: Exception) {
                Log.e(TAG, "no se pudo iniciar la captura", error)
                stopSelf()
            }
        }
        return START_NOT_STICKY
    }

    private fun startCapture(intent: Intent) {
        val data = captureData(intent) ?: throw IllegalStateException("sin permiso de captura")
        val resultCode = intent.getIntExtra(EXTRA_RESULT_CODE, 0)
        val manager = getSystemService(MediaProjectionManager::class.java)
        val granted = manager.getMediaProjection(resultCode, data)
            ?: throw IllegalStateException("Android rechazó la captura")
        granted.registerCallback(projectionCallback, handler)
        projection = granted

        val capture = currentSize()
        val started = Encoder(capture.width, capture.height)
        encoder = started
        size = capture
        display = granted.createVirtualDisplay(
            "puente",
            capture.width,
            capture.height,
            capture.dpi,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
            started.surface,
            null,
            null,
        )
        running = true
        Native.onMirrorState(true)
        startAudio(granted)
        keepScreenOn()
    }

    @Suppress("DEPRECATION")
    private fun keepScreenOn() {
        val power = getSystemService(PowerManager::class.java)
        screenLock = power.newWakeLock(PowerManager.SCREEN_BRIGHT_WAKE_LOCK, "puente:compartiendo").apply {
            setReferenceCounted(false)
        }
        screenKeeper.run()
    }

    private fun releaseScreen() {
        handler.removeCallbacks(screenKeeper)
        screenLock?.takeIf { it.isHeld }?.release()
        screenLock = null
    }

    /** El sonido es opcional: si falla, la pantalla sigue compartiéndose. */
    private fun startAudio(granted: MediaProjection) {
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) return
        try {
            audio = AudioCapture(granted)
        } catch (error: Exception) {
            Log.w(TAG, "no se pudo capturar el audio", error)
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        val current = display ?: return
        val capture = currentSize()
        if (capture == size) return

        current.surface = null
        encoder?.stop()
        val replaced = Encoder(capture.width, capture.height)
        encoder = replaced
        size = capture
        current.resize(capture.width, capture.height, capture.dpi)
        current.surface = replaced.surface
    }

    /** La pantalla en Rust solo se puede crear una vez por proceso. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
        Process.killProcess(Process.myPid())
    }

    override fun onDestroy() {
        running = false
        releaseScreen()
        Native.onMirrorState(false)
        audio?.stop()
        audio = null
        display?.release()
        display = null
        encoder?.stop()
        encoder = null
        projection?.unregisterCallback(projectionCallback)
        projection?.stop()
        projection = null
        super.onDestroy()
    }

    @Suppress("DEPRECATION")
    private fun currentSize(): CaptureSize {
        val metrics = DisplayMetrics()
        getSystemService(DisplayManager::class.java)
            .getDisplay(Display.DEFAULT_DISPLAY)
            .getRealMetrics(metrics)
        val longest = max(metrics.widthPixels, metrics.heightPixels)
        val scale = min(1.0, MAX_DIMENSION.toDouble() / longest)
        return CaptureSize(even(metrics.widthPixels * scale), even(metrics.heightPixels * scale), metrics.densityDpi)
    }

    private fun even(value: Double) = (value / 2).roundToInt() * 2

    @Suppress("DEPRECATION")
    private fun captureData(intent: Intent): Intent? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(EXTRA_DATA, Intent::class.java)
        } else {
            intent.getParcelableExtra(EXTRA_DATA)
        }

    private fun buildNotification(): Notification {
        val channel = NotificationChannel(CHANNEL_ID, "Pantalla compartida", NotificationManager.IMPORTANCE_LOW)
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)

        val stopIntent = PendingIntent.getService(
            this,
            0,
            Intent(this, MirrorService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val stopAction = Notification.Action.Builder(
            Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel),
            "Detener",
            stopIntent,
        ).build()

        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_menu_view)
            .setContentTitle("Puente está compartiendo tu pantalla")
            .setOngoing(true)
            .addAction(stopAction)
            .build()
    }

    companion object {
        /** Verdadero mientras se está compartiendo la pantalla. */
        @Volatile
        var running = false
            private set

        private const val TAG = "puente"
        private const val ACTION_STOP = "dev.puente.app.STOP"
        private const val EXTRA_RESULT_CODE = "result_code"
        private const val EXTRA_DATA = "data"
        private const val CHANNEL_ID = "mirror"
        private const val NOTIFICATION_ID = 1
        private const val MAX_DIMENSION = 1920
        private const val SCREEN_CHECK_MILLISECONDS = 3000L

        fun startIntent(context: Context, resultCode: Int, data: Intent): Intent =
            Intent(context, MirrorService::class.java)
                .putExtra(EXTRA_RESULT_CODE, resultCode)
                .putExtra(EXTRA_DATA, data)
    }
}
