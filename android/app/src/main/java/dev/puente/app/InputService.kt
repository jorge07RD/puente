package dev.puente.app

import android.accessibilityservice.AccessibilityService
import android.graphics.PixelFormat
import android.graphics.PointF
import android.hardware.display.DisplayManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.util.Log
import android.view.Display
import android.view.View
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction
import org.json.JSONException
import org.json.JSONObject
import kotlin.concurrent.thread

/** Ejecuta en el móvil las órdenes que llegan de la PC. */
class InputService : AccessibilityService() {
    private val handler = Handler(Looper.getMainLooper())
    private val touch by lazy { TouchInjector(this, handler) }

    @Volatile
    private var running = false

    /** Ventana invisible que impide que la pantalla se apague y se bloquee. */
    private var awake: View? = null

    override fun onServiceConnected() {
        if (running) return
        running = true
        instance = this
        PhoneRuntime.ensureStarted(this)
        Native.onInputState(true)
        thread(name = "puente-input") {
            while (running) {
                val input = Native.waitInput(WAIT_MILLISECONDS) ?: continue
                handler.post { handle(input) }
            }
        }
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) = Unit

    override fun onInterrupt() = Unit

    override fun onDestroy() {
        running = false
        if (instance === this) instance = null
        keepScreenOn(false)
        Native.onInputState(false)
        super.onDestroy()
    }

    private fun handle(text: String) {
        try {
            val input = JSONObject(text)
            when (input.getString("input")) {
                "touch" -> handleTouch(input)
                "scroll" -> handleScroll(input)
                "navigate" -> handleNavigate(input.getString("action"))
                "text" -> insertText(input.getString("text"))
                "key" -> handleKey(input.getString("key"))
                "cancel" -> touch.cancel()
            }
        } catch (error: JSONException) {
            Log.w(TAG, "orden no válida", error)
        }
    }

    private fun handleTouch(input: JSONObject) {
        val point = pixel(input)
        when (input.getString("phase")) {
            "down" -> touch.down(point)
            "move" -> touch.move(point)
            "up" -> touch.up(point)
        }
    }

    private fun handleScroll(input: JSONObject) {
        val from = pixel(input)
        val height = screen().heightPixels.toFloat()
        // Rueda hacia arriba: el dedo baja y el contenido sube.
        val distance = input.getDouble("lines").toFloat() * height * SCROLL_FRACTION_PER_LINE
        val destination = (from.y + distance).coerceIn(1f, height - 2f)
        touch.scroll(from, PointF(from.x, destination))
    }

    private fun handleNavigate(action: String) {
        val global = when (action) {
            "back" -> GLOBAL_ACTION_BACK
            "home" -> GLOBAL_ACTION_HOME
            "recents" -> GLOBAL_ACTION_RECENTS
            else -> return
        }
        performGlobalAction(global)
    }

    private fun handleKey(key: String) {
        when (key) {
            "backspace" -> deleteBackward()
            "enter" -> pressEnter()
        }
    }

    private fun pressEnter() {
        val node = focusedEditable() ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R &&
            node.performAction(AccessibilityAction.ACTION_IME_ENTER.id)
        ) {
            return
        }
        insertText("\n")
    }

    private fun insertText(text: String) {
        edit { current, start, end ->
            current.replaceRange(start, end, text) to start + text.length
        }
    }

    private fun deleteBackward() {
        edit { current, start, end ->
            when {
                start != end -> current.removeRange(start, end) to start
                start > 0 -> {
                    val previous = current.offsetByCodePoints(start, -1)
                    current.removeRange(previous, start) to previous
                }
                else -> current to 0
            }
        }
    }

    /** Aplica un cambio al campo enfocado y deja el cursor donde corresponde. */
    private fun edit(change: (String, Int, Int) -> Pair<String, Int>) {
        val node = focusedEditable() ?: return
        val current = if (node.isShowingHintText) "" else node.text?.toString().orEmpty()

        val selectionStart = node.textSelectionStart
        val selectionEnd = node.textSelectionEnd
        val hasSelection = selectionStart >= 0 && selectionEnd >= 0
        val first = if (hasSelection) minOf(selectionStart, selectionEnd) else current.length
        val second = if (hasSelection) maxOf(selectionStart, selectionEnd) else current.length
        val (updated, cursor) = change(current, first.coerceAtMost(current.length), second.coerceAtMost(current.length))

        node.performAction(
            AccessibilityNodeInfo.ACTION_SET_TEXT,
            Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, updated)
            },
        )
        node.performAction(
            AccessibilityNodeInfo.ACTION_SET_SELECTION,
            Bundle().apply {
                putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, cursor)
                putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, cursor)
            },
        )
    }

    private fun focusedEditable(): AccessibilityNodeInfo? =
        findFocus(AccessibilityNodeInfo.FOCUS_INPUT)?.takeIf { it.isEditable }

    private fun pixel(input: JSONObject): PointF {
        val metrics = screen()
        return PointF(
            input.getDouble("x").toFloat() * (metrics.widthPixels - 1),
            input.getDouble("y").toFloat() * (metrics.heightPixels - 1),
        )
    }

    @Suppress("DEPRECATION")
    private fun screen(): DisplayMetrics {
        val metrics = DisplayMetrics()
        getSystemService(DisplayManager::class.java)
            .getDisplay(Display.DEFAULT_DISPLAY)
            .getRealMetrics(metrics)
        return metrics
    }

    /** Devuelve falso si Android no dejó crear la ventana. */
    fun keepScreenOn(on: Boolean): Boolean {
        val windows = getSystemService(WindowManager::class.java)
        if (!on) {
            awake?.let { windows.removeView(it) }
            awake = null
            return true
        }
        if (awake != null) return true
        val params = WindowManager.LayoutParams(
            1,
            1,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON,
            PixelFormat.TRANSLUCENT,
        )
        val view = View(this)
        return try {
            windows.addView(view, params)
            awake = view
            true
        } catch (error: RuntimeException) {
            Log.w(TAG, "no se pudo mantener la pantalla encendida", error)
            false
        }
    }

    companion object {
        /** El servicio activo, si el usuario activó la Accesibilidad. Solo desde el hilo principal. */
        var instance: InputService? = null
            private set

        private const val TAG = "puente"
        private const val WAIT_MILLISECONDS = 500
        private const val SCROLL_FRACTION_PER_LINE = 0.12f
    }
}
