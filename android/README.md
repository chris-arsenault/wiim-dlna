# Airwave Control (Android widget)

A minimal native Android home-screen widget for controlling an Airwave server.
Player controls only — no library browsing, no voice.

## Controls

- The header shows every enabled speaker in the shared playing group.
- Volume down / volume up (5% steps).
- Play / pause (toggles based on current state).
- Next track.

## Configuration

Launch the app once and set the Airwave server URL (the LAN address or public
hostname of your Airwave server). The widget calls the same `/api` endpoints the
web UI uses, so point it at a reachable Airwave instance.

For a LAN endpoint, leave **Use Cognito authentication** disabled. For the
public API, enable it and sign in with the same Cognito username, password, and
authenticator code used by the web UI. The password is never stored. ID and
refresh tokens are encrypted with an Android Keystore key, and the widget
refreshes expired ID tokens automatically.

The release build is configured for Airwave's Cognito app client. Custom builds
can override the public pool identifiers with the
`AIRWAVE_COGNITO_USER_POOL_ID` and `AIRWAVE_COGNITO_CLIENT_ID` Gradle properties
or environment variables.

## Build

```bash
cd android
gradle assembleDebug      # or assembleRelease for the F-Droid unsigned artifact
```

Direct release signing reads `ANDROID_SIGNING_STORE_FILE`,
`ANDROID_SIGNING_STORE_PASSWORD`, `ANDROID_SIGNING_KEY_ALIAS`, and
`ANDROID_SIGNING_KEY_PASSWORD` from the environment (used by CI). Without them,
`assembleRelease` produces the unsigned APK expected by the F-Droid publisher.

## Distribution

CI publishes signed releases to the shared Ahara F-Droid repository. Add
`https://fdroid.services.ahara.io/repo` in the F-Droid client.
