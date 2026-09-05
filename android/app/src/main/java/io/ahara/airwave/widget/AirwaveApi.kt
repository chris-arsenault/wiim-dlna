package io.ahara.airwave.widget

import org.json.JSONArray
import org.json.JSONObject
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import kotlin.math.max
import kotlin.math.min

data class AirwaveDevice(
    val id: String,
    val name: String,
    val enabled: Boolean,
    val volume: Double,
    val avTransport: Boolean,
    val groupId: String?,
    val isMaster: Boolean,
)

data class PlaybackState(
    val playing: Boolean,
    val title: String?,
    val artist: String?,
    val album: String?,
    /** The album/playlist/source the current track is playing from. */
    val source: String?,
)

class AirwaveApi(
    private val baseUrl: String,
    private val apiToken: String = "",
) {
    fun devices(): List<AirwaveDevice> {
        val json = JSONArray(request("GET", "/api/devices"))
        return buildList {
            for (i in 0 until json.length()) {
                val item = json.getJSONObject(i)
                if (item.optBoolean("enabled", true) && item.optString("device_type") == "wiim") {
                    add(
                        AirwaveDevice(
                            id = item.getString("id"),
                            name = item.optString("name", item.getString("id")),
                            enabled = true,
                            volume = item.optDouble("volume", 0.0),
                            avTransport = item.optJSONObject("capabilities")
                                ?.optBoolean("av_transport", false) == true,
                            groupId = if (item.isNull("group_id")) {
                                null
                            } else {
                                item.optString("group_id").ifBlank { null }
                            },
                            isMaster = item.optBoolean("is_master", false),
                        )
                    )
                }
            }
        }
    }

    fun playback(): PlaybackState {
        val json = JSONObject(request("GET", "/api/playback"))
        val track = json.optJSONObject("current_track")
        val session = json.optJSONObject("session")
        return PlaybackState(
            playing = json.optBoolean("playing", false),
            title = track?.optString("title")?.ifBlank { null },
            artist = track?.optString("artist")?.ifBlank { null },
            album = track?.optString("album")?.ifBlank { null },
            source = session?.optString("label")?.ifBlank { null },
        )
    }

    fun pause() {
        request("POST", "/api/playback/pause")
    }

    fun resume() {
        request("POST", "/api/playback/resume")
    }

    fun next() {
        request("POST", "/api/playback/next")
    }

    fun setVolume(volume: Double) {
        val clamped = max(0.0, min(1.0, volume))
        request(
            "POST",
            "/api/playback/volume",
            JSONObject().put("volume", clamped).toString(),
        )
    }

    private fun request(method: String, path: String, body: String? = null): String {
        val url = URL(baseUrl.trimEnd('/') + path)
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = method
            connectTimeout = 5_000
            readTimeout = 8_000
            setRequestProperty("Accept", "application/json")
            if (apiToken.isNotBlank()) {
                setRequestProperty("Authorization", "Bearer $apiToken")
            }
            if (body != null) {
                doOutput = true
                setRequestProperty("Content-Type", "application/json")
            }
        }
        if (body != null) {
            OutputStreamWriter(conn.outputStream, Charsets.UTF_8).use { it.write(body) }
        }
        val code = conn.responseCode
        val stream = if (code in 200..299) conn.inputStream else conn.errorStream
        val text = stream?.bufferedReader(Charsets.UTF_8)?.use { it.readText() }.orEmpty()
        if (code !in 200..299) {
            throw IllegalStateException("Airwave HTTP $code: ${text.take(160)}")
        }
        return text
    }

}
