package io.ahara.airwave.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import io.ahara.airwave.R

class AirwaveWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, widgetIds: IntArray) {
        widgetIds.forEach { updateWidget(context, manager, it, "Loading…") }
        refreshAsync(context)
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        when (intent.action) {
            ACTION_PLAY_PAUSE,
            ACTION_NEXT,
            ACTION_VOLUME_UP,
            ACTION_VOLUME_DOWN -> {
                val result = goAsync()
                Thread {
                    try {
                        handleAction(context, intent.action.orEmpty())
                    } catch (e: Exception) {
                        updateAll(context, "Error: ${e.message ?: "unknown"}")
                    } finally {
                        result.finish()
                    }
                }.start()
            }
        }
    }

    companion object {
        const val ACTION_PLAY_PAUSE = "io.ahara.airwave.action.PLAY_PAUSE"
        const val ACTION_NEXT = "io.ahara.airwave.action.NEXT"
        const val ACTION_VOLUME_UP = "io.ahara.airwave.action.VOLUME_UP"
        const val ACTION_VOLUME_DOWN = "io.ahara.airwave.action.VOLUME_DOWN"
        private const val VOLUME_STEP = 0.05

        fun refreshAsync(context: Context) {
            Thread {
                try {
                    refresh(context)
                } catch (e: Exception) {
                    updateAll(context, "Error: ${e.message ?: "unknown"}")
                }
            }.start()
        }

        fun updateAll(context: Context, status: String? = null) {
            val manager = AppWidgetManager.getInstance(context)
            val ids = manager.getAppWidgetIds(ComponentName(context, AirwaveWidgetProvider::class.java))
            ids.forEach { updateWidget(context, manager, it, status) }
        }

        private fun handleAction(context: Context, action: String) {
            val api = apiOrThrow(context)
            when (action) {
                ACTION_PLAY_PAUSE -> {
                    val state = api.playback()
                    if (state.playing) api.pause() else api.resume()
                    AirwavePrefs.setPlaying(context, !state.playing)
                    updateAll(context, if (state.playing) "Paused" else "Playing")
                }
                ACTION_NEXT -> {
                    api.next()
                    updateAll(context, "Next track")
                }
                ACTION_VOLUME_UP -> adjustVolume(context, api, VOLUME_STEP)
                ACTION_VOLUME_DOWN -> adjustVolume(context, api, -VOLUME_STEP)
            }
            refresh(context)
        }

        private fun refresh(context: Context) {
            val api = apiOrThrow(context)
            val devices = api.devices()
            val state = runCatching { api.playback() }.getOrNull()
            AirwavePrefs.setPlaying(context, state?.playing == true)
            AirwavePrefs.setNowPlaying(
                context,
                state?.title ?: "Nothing playing",
                playbackSubtitle(state, devices),
            )
            AirwavePrefs.setOutputNames(context, outputLabel(devices))
            val master = playbackDevice(devices)
            val stateText = if (state?.playing == true) "Playing" else "Paused"
            val status = if (master != null) {
                "$stateText • vol ${Math.round(master.volume * 100)}"
            } else {
                "All speakers off"
            }
            updateAll(context, status)
        }

        private fun playbackSubtitle(
            state: PlaybackState?,
            devices: List<AirwaveDevice>,
        ): String {
            if (state?.title == null) return outputLabel(devices)
            return listOfNotNull(state.artist, state.album ?: state.source)
                .takeIf { it.isNotEmpty() }
                ?.joinToString(" • ")
                ?: outputLabel(devices)
        }

        private fun adjustVolume(context: Context, api: AirwaveApi, delta: Double) {
            val device = playbackDevice(api.devices())
                ?: throw IllegalStateException("All speakers are off")
            api.setVolume(device.volume + delta)
            val direction = if (delta > 0) "up" else "down"
            updateAll(context, "Volume $direction")
        }

        private fun playbackDevice(devices: List<AirwaveDevice>): AirwaveDevice? =
            devices.filter { it.avTransport }
                .sortedBy { it.id }
                .let { outputs ->
                    outputs.firstOrNull { it.isMaster && it.groupId == it.id }
                        ?: outputs.firstOrNull { it.groupId == null }
                        ?: outputs.firstOrNull()
                }

        private fun outputLabel(devices: List<AirwaveDevice>): String =
            if (devices.isEmpty()) "All speakers off" else devices.joinToString(" + ") { it.name }

        private fun apiOrThrow(context: Context): AirwaveApi {
            val serverUrl = AirwavePrefs.serverUrl(context)
            if (serverUrl.isBlank()) {
                throw IllegalStateException("Open app to set server URL")
            }
            val token = if (AirwavePrefs.authRequired(context)) {
                AirwaveAuthManager.idToken(context)
            } else {
                ""
            }
            return AirwaveApi(serverUrl, token)
        }

        private fun updateWidget(
            context: Context,
            manager: AppWidgetManager,
            widgetId: Int,
            status: String?,
        ) {
            val views = RemoteViews(context.packageName, R.layout.airwave_widget)
            val subtitle = AirwavePrefs.nowSubtitle(context).ifBlank { status ?: "Ready" }
            views.setTextViewText(R.id.now_title, AirwavePrefs.nowTitle(context).ifBlank { "Airwave" })
            views.setTextViewText(R.id.now_subtitle, subtitle)
            setOutputText(context, views)
            views.setImageViewResource(
                R.id.play_pause,
                if (AirwavePrefs.playing(context)) R.drawable.ic_pause else R.drawable.ic_play,
            )
            views.setOnClickPendingIntent(R.id.volume_down, pendingIntent(context, ACTION_VOLUME_DOWN))
            views.setOnClickPendingIntent(R.id.play_pause, pendingIntent(context, ACTION_PLAY_PAUSE))
            views.setOnClickPendingIntent(R.id.next, pendingIntent(context, ACTION_NEXT))
            views.setOnClickPendingIntent(R.id.volume_up, pendingIntent(context, ACTION_VOLUME_UP))
            manager.updateAppWidget(widgetId, views)
        }

        private fun setOutputText(context: Context, views: RemoteViews) {
            val serverUrl = AirwavePrefs.serverUrl(context)
            if (serverUrl.isBlank()) {
                views.setTextViewText(R.id.device_name, "Set server URL")
                return
            }
            views.setTextViewText(
                R.id.device_name,
                AirwavePrefs.outputNames(context).ifBlank { "All speakers off" },
            )
        }

        private fun pendingIntent(context: Context, action: String): PendingIntent {
            val intent = Intent(context, AirwaveWidgetProvider::class.java).setAction(action)
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            return PendingIntent.getBroadcast(context, action.hashCode(), intent, flags)
        }
    }
}
