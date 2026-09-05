package io.ahara.airwave.widget

import android.content.Context

object AirwavePrefs {
    private const val PREFS = "airwave-control"
    private const val KEY_SERVER_URL = "server_url"
    private const val KEY_API_TOKEN = "api_token"
    private const val KEY_AUTH_REQUIRED = "auth_required"
    private const val KEY_OUTPUT_NAMES = "output_names"
    private const val KEY_PLAYING = "playing"
    private const val KEY_NOW_TITLE = "now_title"
    private const val KEY_NOW_SUBTITLE = "now_subtitle"

    fun serverUrl(context: Context): String =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_SERVER_URL, "") ?: ""

    fun setServerUrl(context: Context, value: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_SERVER_URL, value.trim().trimEnd('/'))
            .apply()
    }

    fun authRequired(context: Context): Boolean {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        if (prefs.contains(KEY_AUTH_REQUIRED)) return prefs.getBoolean(KEY_AUTH_REQUIRED, false)
        return !prefs.getString(KEY_API_TOKEN, "").isNullOrBlank()
    }

    fun setAuthRequired(context: Context, value: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_AUTH_REQUIRED, value)
            .remove(KEY_API_TOKEN)
            .apply()
    }

    fun outputNames(context: Context): String =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_OUTPUT_NAMES, "") ?: ""

    fun setOutputNames(context: Context, names: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_OUTPUT_NAMES, names)
            .apply()
    }

    fun playing(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_PLAYING, false)

    fun setPlaying(context: Context, value: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_PLAYING, value)
            .apply()
    }

    fun nowTitle(context: Context): String =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_NOW_TITLE, "") ?: ""

    fun nowSubtitle(context: Context): String =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_NOW_SUBTITLE, "") ?: ""

    fun setNowPlaying(context: Context, title: String, subtitle: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_NOW_TITLE, title)
            .putString(KEY_NOW_SUBTITLE, subtitle)
            .apply()
    }
}
