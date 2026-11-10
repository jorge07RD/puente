package dev.puente.app

import android.app.Notification
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.media.MediaMetadata
import android.media.session.MediaController
import android.media.session.MediaSessionManager
import android.media.session.PlaybackState
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification

/** Lee las notificaciones y controla la música del móvil. */
class NotifService : NotificationListenerService() {
    private class CallControls(val answer: PendingIntent?, val decline: PendingIntent?)

    private val replyActions = HashMap<String, Notification.Action>()
    private val calls = HashMap<String, CallControls>()
    private val handler = Handler(Looper.getMainLooper())
    private var watched: List<MediaController> = emptyList()
    private var controller: MediaController? = null

    private val mediaCallback = object : MediaController.Callback() {
        override fun onPlaybackStateChanged(state: PlaybackState?) = publishMedia()

        override fun onMetadataChanged(metadata: MediaMetadata?) = publishMedia()

        override fun onSessionDestroyed() = refreshSessions()
    }

    private val sessionsListener = MediaSessionManager.OnActiveSessionsChangedListener { refreshSessions() }

    override fun onListenerConnected() {
        PhoneRuntime.ensureStarted(this)
        instance = this
        Native.onNotificationAccess(true)
        activeNotifications?.forEach(::report)

        val manager = getSystemService(MediaSessionManager::class.java)
        manager.addOnActiveSessionsChangedListener(sessionsListener, component())
        refreshSessions()
    }

    override fun onListenerDisconnected() {
        instance = null
        Native.onNotificationAccess(false)
        getSystemService(MediaSessionManager::class.java).removeOnActiveSessionsChangedListener(sessionsListener)
        watched.forEach { it.unregisterCallback(mediaCallback) }
        watched = emptyList()
        controller = null
        Native.onMedia(null, null, false)
    }

    override fun onNotificationPosted(notification: StatusBarNotification) = report(notification)

    override fun onNotificationRemoved(notification: StatusBarNotification) {
        replyActions.remove(notification.key)
        if (calls.remove(notification.key) != null) {
            Native.onCallEnded(notification.key)
        }
        Native.onNotificationRemoved(notification.key)
    }

    private fun report(item: StatusBarNotification) {
        val notification = item.notification
        if (item.packageName == packageName || reportCall(item)) return
        val isSummary = notification.flags and Notification.FLAG_GROUP_SUMMARY != 0
        if (!item.isClearable || isSummary) return

        val extras = notification.extras
        val title = extras.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (extras.getCharSequence(Notification.EXTRA_BIG_TEXT) ?: extras.getCharSequence(Notification.EXTRA_TEXT))
            ?.toString()
            .orEmpty()
        if (title.isEmpty() && text.isEmpty()) return

        val reply = replyAction(notification)
        if (reply != null) replyActions[item.key] = reply else replyActions.remove(item.key)
        Native.onNotification(item.key, appLabel(item.packageName), title, text, reply != null)
    }

    private fun replyAction(notification: Notification): Notification.Action? {
        val candidates = notification.actions.orEmpty().filter { action ->
            action.remoteInputs.orEmpty().any { it.allowFreeFormInput }
        }
        return candidates.firstOrNull { it.semanticAction == Notification.Action.SEMANTIC_ACTION_REPLY }
            ?: candidates.firstOrNull()
    }

    /** Envía la respuesta por el mismo camino que usa el propio Android. */
    fun reply(key: String, text: String) {
        val action = replyActions[key]
        val inputs = action?.remoteInputs
        val delivered = action != null && inputs != null && try {
            val results = Bundle()
            inputs.filter { it.allowFreeFormInput }.forEach { results.putCharSequence(it.resultKey, text) }
            val intent = Intent().addFlags(Intent.FLAG_RECEIVER_FOREGROUND)
            RemoteInput.addResultsToIntent(inputs, intent, results)
            action.actionIntent.send(this, 0, intent)
            true
        } catch (_: PendingIntent.CanceledException) {
            false
        }
        Native.onReplyResult(key, delivered)
    }

    /**
     * Las llamadas, de teléfono o de cualquier app, se anuncian con una notificación
     * de categoría llamada. Devuelve verdadero si la notificación era de ese tipo.
     */
    private fun reportCall(item: StatusBarNotification): Boolean {
        val notification = item.notification
        if (notification.category != Notification.CATEGORY_CALL) return false

        if (isIncomingCall(notification)) {
            val controls = callControls(notification)
            calls[item.key] = controls
            Native.onCall(
                item.key,
                appLabel(item.packageName),
                callerName(notification),
                controls.answer != null,
                controls.decline != null,
            )
        } else if (calls.remove(item.key) != null) {
            Native.onCallEnded(item.key)
        }
        return true
    }

    private fun isIncomingCall(notification: Notification): Boolean =
        when (notification.extras.getInt(EXTRA_CALL_TYPE, CALL_TYPE_UNKNOWN)) {
            CALL_TYPE_INCOMING -> true
            CALL_TYPE_UNKNOWN -> notification.fullScreenIntent != null
            else -> false
        }

    private fun callerName(notification: Notification): String =
        notification.extras.getCharSequence(Notification.EXTRA_TITLE)?.toString()?.takeIf { it.isNotBlank() }
            ?: PhoneTexts.unknownNumber

    private fun callControls(notification: Notification): CallControls {
        val actions = notification.actions.orEmpty()
        fun matching(words: Regex) = actions.firstOrNull { it.title?.contains(words) == true }?.actionIntent
        return CallControls(
            answer = pendingExtra(notification, EXTRA_ANSWER_INTENT) ?: matching(ANSWER_WORDS),
            decline = pendingExtra(notification, EXTRA_DECLINE_INTENT) ?: matching(DECLINE_WORDS),
        )
    }

    @Suppress("DEPRECATION")
    private fun pendingExtra(notification: Notification, name: String): PendingIntent? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notification.extras.getParcelable(name, PendingIntent::class.java)
        } else {
            notification.extras.getParcelable(name)
        }

    fun handleCall(key: String, answer: Boolean) {
        val controls = calls[key] ?: return
        try {
            (if (answer) controls.answer else controls.decline)?.send()
        } catch (_: PendingIntent.CanceledException) {
            Native.log("no se pudo actuar sobre la llamada")
        }
    }

    private fun appLabel(packageName: String): String =
        try {
            packageManager.getApplicationLabel(packageManager.getApplicationInfo(packageName, 0)).toString()
        } catch (_: PackageManager.NameNotFoundException) {
            packageName
        }

    private fun component() = ComponentName(this, NotifService::class.java)

    /** Se vigilan todas las sesiones: cualquiera puede empezar a sonar en cualquier momento. */
    private fun refreshSessions() {
        val sessions = getSystemService(MediaSessionManager::class.java).getActiveSessions(component())
        if (sessions.map { it.sessionToken } != watched.map { it.sessionToken }) {
            watched.forEach { it.unregisterCallback(mediaCallback) }
            sessions.forEach { it.registerCallback(mediaCallback, handler) }
            watched = sessions
            Native.log("música: " + sessions.joinToString { "${it.packageName} ${it.playbackState?.state} ${titleOf(it) != null}" })
        }
        publishMedia()
    }

    private fun isPlaying(session: MediaController): Boolean =
        session.playbackState?.state == PlaybackState.STATE_PLAYING ||
            session.playbackState?.state == PlaybackState.STATE_BUFFERING

    private fun titleOf(session: MediaController): String? {
        val metadata = session.metadata ?: return null
        return (metadata.getString(MediaMetadata.METADATA_KEY_TITLE)
            ?: metadata.getString(MediaMetadata.METADATA_KEY_DISPLAY_TITLE))
            ?.takeIf { it.isNotBlank() }
    }

    private fun artistOf(session: MediaController): String {
        val metadata = session.metadata ?: return ""
        return metadata.getString(MediaMetadata.METADATA_KEY_ARTIST)
            ?: metadata.getString(MediaMetadata.METADATA_KEY_DISPLAY_SUBTITLE)
            ?: ""
    }

    /** Lo que suena gana a lo pausado; una sesión sin título no sirve de nada en la PC. */
    private fun bestSession(): MediaController? =
        watched
            .filter { titleOf(it) != null }
            .maxByOrNull { if (isPlaying(it)) 1 else 0 }

    private fun publishMedia() {
        val current = bestSession()
        controller = current
        if (current == null) {
            Native.onMedia(null, null, false)
            return
        }
        Native.onMedia(titleOf(current), artistOf(current), isPlaying(current))
    }

    fun controlMedia(action: String) {
        val current = controller ?: return
        val controls = current.transportControls
        when (action) {
            "play_pause" -> if (isPlaying(current)) controls.pause() else controls.play()
            "next" -> controls.skipToNext()
            "previous" -> controls.skipToPrevious()
        }
    }

    companion object {
        @Volatile
        var instance: NotifService? = null
            private set

        // Los mismos valores que usa Notification.CallStyle en Android 12 y posteriores.
        private const val EXTRA_CALL_TYPE = "android.callType"
        private const val EXTRA_ANSWER_INTENT = "android.answerIntent"
        private const val EXTRA_DECLINE_INTENT = "android.declineIntent"
        private const val CALL_TYPE_UNKNOWN = 0
        private const val CALL_TYPE_INCOMING = 1

        private val ANSWER_WORDS = Regex("contestar|responder|atender|aceptar|answer|accept", RegexOption.IGNORE_CASE)
        private val DECLINE_WORDS = Regex("rechazar|colgar|ignorar|decline|reject|dismiss", RegexOption.IGNORE_CASE)
    }
}
