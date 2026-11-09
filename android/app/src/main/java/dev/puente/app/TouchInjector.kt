package dev.puente.app

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.accessibilityservice.GestureDescription.StrokeDescription
import android.graphics.Path
import android.graphics.PointF
import android.os.Handler

/**
 * Convierte el flujo de toques de la PC en gestos. Android solo admite un gesto
 * en curso, así que los puntos se acumulan y salen por tramos encadenados: el
 * dedo sigue "apoyado" entre un tramo y el siguiente.
 */
internal class TouchInjector(
    private val service: AccessibilityService,
    private val handler: Handler,
) {
    private enum class Phase { DOWN, MOVE, UP }

    private data class Event(val phase: Phase, val point: PointF)

    private data class Scroll(val from: PointF, val to: PointF)

    private val events = ArrayDeque<Event>()
    private val scrolls = ArrayDeque<Scroll>()
    private var stroke: StrokeDescription? = null
    private var last: PointF? = null
    private var inFlight = false

    private val callback = object : AccessibilityService.GestureResultCallback() {
        override fun onCompleted(gestureDescription: GestureDescription?) {
            finishSegment()
        }

        override fun onCancelled(gestureDescription: GestureDescription?) {
            stroke = null
            finishSegment()
        }
    }

    fun down(point: PointF) = enqueue(Event(Phase.DOWN, point))

    fun move(point: PointF) = enqueue(Event(Phase.MOVE, point))

    fun up(point: PointF) = enqueue(Event(Phase.UP, point))

    fun scroll(from: PointF, to: PointF) {
        scrolls.addLast(Scroll(from, to))
        pump()
    }

    /** Levanta el dedo si había un gesto a medias. */
    fun cancel() {
        scrolls.clear()
        val point = events.lastOrNull()?.point ?: last ?: return
        if (stroke != null || events.isNotEmpty()) {
            enqueue(Event(Phase.UP, point))
        }
    }

    private fun enqueue(event: Event) {
        events.addLast(event)
        pump()
    }

    private fun finishSegment() {
        inFlight = false
        pump()
    }

    private fun pump() {
        if (inFlight) return
        val current = stroke
        when {
            current != null -> continueStroke(current)
            events.isNotEmpty() -> beginStroke()
            scrolls.isNotEmpty() -> dispatchScroll(scrolls.removeFirst())
        }
    }

    private fun beginStroke() {
        // Tras un gesto cancelado pueden quedar restos; se espera al próximo toque.
        while (events.isNotEmpty() && events.first().phase != Phase.DOWN) {
            events.removeFirst()
        }
        val start = events.removeFirstOrNull()?.point ?: return
        val path = Path().apply { moveTo(start.x, start.y) }
        val first = StrokeDescription(path, 0, MIN_SEGMENT_MILLISECONDS, true)
        last = start
        dispatch(first, first)
    }

    private fun continueStroke(current: StrokeDescription) {
        if (events.isEmpty()) return
        val origin = last ?: return
        val path = Path().apply { moveTo(origin.x, origin.y) }
        var end = origin
        var points = 0
        var finished = false

        while (!finished && events.isNotEmpty() && events.first().phase != Phase.DOWN) {
            val event = events.removeFirst()
            path.lineTo(event.point.x, event.point.y)
            end = event.point
            points++
            finished = event.phase == Phase.UP
        }
        if (points == 0) {
            // Un toque nuevo sin haber soltado el anterior: se suelta primero.
            finished = true
        }

        val duration = maxOf(MIN_SEGMENT_MILLISECONDS, points * MILLISECONDS_PER_POINT)
        val next = current.continueStroke(path, 0, duration, !finished)
        last = end
        dispatch(next, if (finished) null else next)
    }

    private fun dispatchScroll(scroll: Scroll) {
        val path = Path().apply {
            moveTo(scroll.from.x, scroll.from.y)
            lineTo(scroll.to.x, scroll.to.y)
        }
        dispatch(StrokeDescription(path, 0, SCROLL_MILLISECONDS, false), null)
    }

    private fun dispatch(segment: StrokeDescription, nextStroke: StrokeDescription?) {
        val gesture = GestureDescription.Builder().addStroke(segment).build()
        stroke = nextStroke
        inFlight = service.dispatchGesture(gesture, callback, handler)
        if (!inFlight) {
            stroke = null
            events.clear()
        }
    }

    private companion object {
        const val MIN_SEGMENT_MILLISECONDS = 10L
        const val MILLISECONDS_PER_POINT = 8L
        const val SCROLL_MILLISECONDS = 150L
    }
}
