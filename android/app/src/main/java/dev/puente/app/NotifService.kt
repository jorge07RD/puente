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
        controller?.unregisterCallback(mediaCallback)
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

    private fun refreshSessions() {
        val sessions = getSystemService(MediaSessionManager::class.java).getActiveSessions(component())
        val chosen = sessions.firstOrNull { it.playbackState?.state == PlaybackState.STATE_PLAYING }
            ?: sessions.firstOrNull()

        if (chosen?.sessionToken != controller?.sessionToken) {
            controller?.unregisterCallback(mediaCallback)
            chosen?.registerCallback(mediaCallback, handler)
            controller = chosen
        }
        publishMedia()
    }

    private fun publishMedia() {
        val current = controller
        val metadata = current?.metadata
        val title = metadata?.getString(MediaMetadata.METADATA_KEY_TITLE)
        if (current == null || title == null) {
            Native.onMedia(null, null, false)
            return
        }
        val artist = metadata.getString(MediaMetadata.METADATA_KEY_ARTIST).orEmpty()
        Native.onMedia(title, artist, current.playbackState?.state == PlaybackState.STATE_PLAYING)
    }

    fun controlMedia(action: String) {
        val current = controller ?: return
        val controls = current.transportControls
        when (action) {
            "play_pause" ->
                if (current.playbackState?.state == PlaybackState.STATE_PLAYING) controls.pause() else controls.play()
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
