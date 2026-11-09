package dev.puente.app

import android.Manifest
import android.app.NativeActivity
import android.content.Intent
import android.content.pm.PackageManager
import android.media.projection.MediaProjectionManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.os.Process
import android.provider.Settings
import android.view.WindowInsets
import kotlin.system.exitProcess

class MainActivity : NativeActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        PhoneRuntime.ensureStarted(this)
        Native.log("MainActivity.onCreate, ya hubo pantalla en este proceso: $uiStarted")
        if (uiStarted) restartInFreshProcess()
        uiStarted = true
        super.onCreate(savedInstanceState)
        reportDisplay()
    }

    /**
     * La interfaz en Rust solo se puede crear una vez por proceso, y los servicios
     * de Accesibilidad y Notificaciones lo mantienen vivo aunque se cierre la app.
     */
    private fun restartInFreshProcess(): Nothing {
        val launch = packageManager.getLaunchIntentForPackage(packageName)
            ?: throw IllegalStateException("la app no tiene pantalla de inicio")
        startActivity(launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
        exitProcess(0)
    }

    /** La interfaz ocupa toda la pantalla y se aparta sola de las barras del sistema. */
    private fun reportDisplay() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
        window.setDecorFitsSystemWindows(false)
        window.decorView.setOnApplyWindowInsetsListener { view, insets ->
            val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            Native.onDisplay(bars.top, bars.bottom, resources.configuration.fontScale)
            view.onApplyWindowInsets(insets)
        }
    }

    override fun onResume() {
        super.onResume()
        Native.log("MainActivity.onResume")
        val power = getSystemService(PowerManager::class.java)
        Native.onBatteryUnrestricted(power.isIgnoringBatteryOptimizations(packageName))
    }

    /** Android solo deja leer el portapapeles a la app que está en primer plano. */
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) PhoneRuntime.shareClipboard(this)
    }

    /** Llamado desde Rust. */
    fun requestCapture() = runOnUiThread {
        val wanted = buildList {
            add(Manifest.permission.RECORD_AUDIO)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) add(Manifest.permission.POST_NOTIFICATIONS)
        }
        val missing = wanted.filter { checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }
        if (missing.isEmpty()) {
            askForCapture()
        } else {
            requestPermissions(missing.toTypedArray(), REQUEST_PERMISSIONS)
        }
    }

    /** Llamado desde Rust. */
    fun stopCapture() = runOnUiThread {
        stopService(Intent(this, MirrorService::class.java))
    }

    /** Llamado desde Rust. */
    fun openAccessibilitySettings() = runOnUiThread {
        startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
    }

    /** Llamado desde Rust. */
    fun openNotificationSettings() = runOnUiThread {
        startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS))
    }

    /** Llamado desde Rust. */
    fun openBatterySettings() = runOnUiThread {
        startActivity(
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                .setData(Uri.parse("package:$packageName")),
        )
    }

    private fun askForCapture() {
        val manager = getSystemService(MediaProjectionManager::class.java)
        startActivityForResult(manager.createScreenCaptureIntent(), REQUEST_CAPTURE)
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == REQUEST_PERMISSIONS) {
            askForCapture()
        }
    }

    @Deprecated("Sigue siendo la API disponible para NativeActivity")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQUEST_CAPTURE && resultCode == RESULT_OK && data != null) {
            startForegroundService(MirrorService.startIntent(this, resultCode, data))
        }
    }

    /** La pantalla en Rust solo se puede crear una vez por proceso. */
    @Deprecated("Sigue siendo la API disponible para NativeActivity")
    override fun onBackPressed() {
        moveTaskToBack(true)
    }

    override fun onDestroy() {
        Native.log("MainActivity.onDestroy, terminando: $isFinishing")
        super.onDestroy()
        if (!isChangingConfigurations && !MirrorService.running) {
            Process.killProcess(Process.myPid())
        }
    }

    private companion object {
        const val REQUEST_CAPTURE = 1
        const val REQUEST_PERMISSIONS = 2

        private var uiStarted = false
    }
}
