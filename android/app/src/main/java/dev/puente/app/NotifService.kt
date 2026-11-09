package dev.puente.app

import android.app.Notification
import android.content.ComponentName
import android.content.pm.PackageManager
import android.media.MediaMetadata
import android.media.session.MediaController
import android.media.session.MediaSessionManager
import android.media.session.PlaybackState
import android.os.Handler
import android.os.Looper
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification

/** Lee las notificaciones y controla la música del móvil. */
class NotifService : NotificationListenerService() {
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
        Native.onNotificationRemoved(notification.key)
    }

    private fun report(item: StatusBarNotification) {
        val notification = item.notification
        val isSummary = notification.flags and Notification.FLAG_GROUP_SUMMARY != 0
        if (item.packageName == packageName || !item.isClearable || isSummary) return

        val extras = notification.extras
        val title = extras.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (extras.getCharSequence(Notification.EXTRA_BIG_TEXT) ?: extras.getCharSequence(Notification.EXTRA_TEXT))
            ?.toString()
            .orEmpty()
        if (title.isEmpty() && text.isEmpty()) return

        Native.onNotification(item.key, appLabel(item.packageName), title, text)
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
    }
}
