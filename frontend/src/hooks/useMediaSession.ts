import { useEffect, useRef, useState, useCallback } from "react";
import { api } from "../api/client";
import { usePlayerStore } from "../stores/playerStore";
import { selectPlaybackDevice, useDeviceStore } from "../stores/deviceStore";
import { setPlaybackVolume } from "./useDeviceVolume";

const VOLUME_STEP = 0.05;
const VOLUME_MIDPOINT = 0.5;

/** Generate a valid 1-second silent WAV as a Blob URL */
function createSilentAudio(): string {
  const sampleRate = 8000;
  const numSamples = sampleRate;
  const buffer = new ArrayBuffer(44 + numSamples * 2);
  const view = new DataView(buffer);
  const write = (off: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i));
  };
  write(0, "RIFF");
  view.setUint32(4, 36 + numSamples * 2, true);
  write(8, "WAVE");
  write(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  write(36, "data");
  view.setUint32(40, numSamples * 2, true);
  return URL.createObjectURL(new Blob([buffer], { type: "audio/wav" }));
}

// Debug log buffer — shared across hook instances
const debugLog: string[] = [];
let debugListeners: Array<(logs: string[]) => void> = [];

function log(msg: string) {
  const ts = new Date().toLocaleTimeString("en", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  debugLog.push(`[${ts}] ${msg}`);
  if (debugLog.length > 30) debugLog.shift();
  debugListeners.forEach((fn) => fn([...debugLog]));
}

/** Subscribe to debug logs — returns unsubscribe function */
export function useMediaSessionDebug(): string[] {
  const [logs, setLogs] = useState<string[]>([...debugLog]);
  useEffect(() => {
    debugListeners.push(setLogs);
    return () => {
      debugListeners = debugListeners.filter((fn) => fn !== setLogs);
    };
  }, []);
  return logs;
}

function handleVolumeChange(
  audioRef: React.RefObject<HTMLAudioElement | null>,
  lastVolumeRef: React.RefObject<number>
) {
  const audio = audioRef.current;
  if (!audio) return;

  const delta = audio.volume - lastVolumeRef.current!;

  // Reset to midpoint to prevent drift
  audio.volume = VOLUME_MIDPOINT;
  lastVolumeRef.current = VOLUME_MIDPOINT;

  if (Math.abs(delta) < 0.001) return;

  log(`volumechange delta=${delta.toFixed(3)}`);

  const device = selectPlaybackDevice(useDeviceStore.getState().devices);
  if (!device) {
    log("no playback output");
    return;
  }

  const direction = delta > 0 ? VOLUME_STEP : -VOLUME_STEP;
  const newVolume = Math.max(0, Math.min(1, device.volume + direction));
  log(`vol ${device.volume.toFixed(2)} -> ${newVolume.toFixed(2)}`);
  setPlaybackVolume(newVolume).catch((e) => log(`setVolume error: ${e}`));
}

function useAudioActivation(
  audioRef: React.RefObject<HTMLAudioElement | null>,
  lastVolumeRef: React.RefObject<number>
) {
  const activatedRef = useRef(false);

  useEffect(() => {
    if (activatedRef.current) return;

    const onVolume = () => handleVolumeChange(audioRef, lastVolumeRef);
    let audio: HTMLAudioElement | null = null;

    const activate = (e: Event) => {
      if (activatedRef.current) return;
      log(`activate attempt via ${e.type}`);

      try {
        if (!audio) {
          const url = createSilentAudio();
          log(`created silent audio blob`);
          audio = new Audio(url);
          audio.loop = true;
          audio.volume = VOLUME_MIDPOINT;
          (audioRef as React.MutableRefObject<HTMLAudioElement | null>).current = audio;
          lastVolumeRef.current = VOLUME_MIDPOINT;

          audio.addEventListener("volumechange", onVolume);
          audio.addEventListener("playing", () => log("audio state: playing"));
          audio.addEventListener("error", (ev) => {
            const err = audio?.error;
            log(
              `audio error: code=${err?.code} msg=${err?.message ?? (ev as ErrorEvent).message ?? "unknown"}`
            );
          });
        }

        const result = audio.play();
        if (result && typeof result.then === "function") {
          result
            .then(() => {
              log("audio.play() resolved — activated!");
              activatedRef.current = true;
              document.removeEventListener("click", activate);
              document.removeEventListener("touchstart", activate);
            })
            .catch((err: Error) => {
              log(`audio.play() rejected: ${err.message} — will retry on next gesture`);
            });
        } else {
          log(`audio.play() returned: ${typeof result}`);
        }
      } catch (err) {
        log(`activate error: ${err}`);
      }
    };

    document.addEventListener("click", activate, { once: false });
    document.addEventListener("touchstart", activate, { once: false });
    log("gesture listeners registered");

    return () => {
      document.removeEventListener("click", activate);
      document.removeEventListener("touchstart", activate);
    };
  }, [audioRef, lastVolumeRef]);
}

function useVisibilityReclaim(audioRef: React.RefObject<HTMLAudioElement | null>) {
  useEffect(() => {
    const onVisibility = () => {
      log(`visibilitychange: ${document.visibilityState}`);
      if (document.visibilityState === "visible" && audioRef.current) {
        const result = audioRef.current.play();
        if (result && typeof result.then === "function") {
          result
            .then(() => log("reclaim play() resolved"))
            .catch((err: Error) => log(`reclaim play() rejected: ${err.message}`));
        }
      }
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [audioRef]);
}

function useMetadataSync() {
  useEffect(() => {
    if (!("mediaSession" in navigator)) {
      log("SKIP metadata: mediaSession not available");
      return;
    }
    log("metadata sync registered");

    const unsub = usePlayerStore.subscribe((state, prev) => {
      const track = state.currentTrack;
      if (!track) {
        navigator.mediaSession.metadata = null;
        navigator.mediaSession.playbackState = "none";
        return;
      }

      if (track.id !== prev.currentTrack?.id) {
        log(`metadata update: ${track.title} - ${track.artist}`);
        const artwork: MediaImage[] = [];
        if (track.id) {
          artwork.push({ src: api.artUrl(track.id), sizes: "512x512", type: "image/jpeg" });
        }
        navigator.mediaSession.metadata = new MediaMetadata({
          title: track.title,
          artist: track.artist ?? undefined,
          album: track.album ?? undefined,
          artwork,
        });
      }

      navigator.mediaSession.playbackState = state.playing ? "playing" : "paused";
    });

    syncInitialMetadata();

    return unsub;
  }, []);
}

function syncInitialMetadata() {
  const { currentTrack, playing } = usePlayerStore.getState();
  if (currentTrack) {
    log(`initial metadata: ${currentTrack.title}`);
    const artwork: MediaImage[] = [];
    if (currentTrack.id) {
      artwork.push({ src: api.artUrl(currentTrack.id), sizes: "512x512", type: "image/jpeg" });
    }
    navigator.mediaSession.metadata = new MediaMetadata({
      title: currentTrack.title,
      artist: currentTrack.artist ?? undefined,
      album: currentTrack.album ?? undefined,
      artwork,
    });
    navigator.mediaSession.playbackState = playing ? "playing" : "paused";
  } else {
    log("no current track on init");
  }
}

function useActionHandlers() {
  useEffect(() => {
    if (!("mediaSession" in navigator)) {
      log("SKIP actions: mediaSession not available");
      return;
    }

    const hasOutput = () => selectPlaybackDevice(useDeviceStore.getState().devices) != null;
    const hasSession = () => usePlayerStore.getState().session !== null;

    const actions = buildActionHandlers(hasOutput, hasSession);

    let registered = 0;
    for (const [action, handler] of actions) {
      try {
        navigator.mediaSession.setActionHandler(action, handler);
        registered++;
      } catch (e) {
        log(`action ${action} unsupported: ${e}`);
      }
    }
    log(`${registered}/${actions.length} action handlers registered`);

    return () => {
      for (const [action] of actions) {
        try {
          navigator.mediaSession.setActionHandler(action, null);
        } catch {
          /* */
        }
      }
    };
  }, []);
}

function buildActionHandlers(
  hasOutput: () => boolean,
  hasSession: () => boolean
): [MediaSessionAction, MediaSessionActionHandler][] {
  return [
    [
      "play",
      () => {
        log("action: play");
        if (hasOutput()) api.resume().catch((e) => log(`play error: ${e}`));
        else log("no playback output for play");
      },
    ],
    [
      "pause",
      () => {
        log("action: pause");
        if (hasOutput()) api.pause().catch((e) => log(`pause error: ${e}`));
        else log("no playback output for pause");
      },
    ],
    [
      "nexttrack",
      () => {
        log("action: next");
        if (!hasOutput()) {
          log("no playback output for next");
          return;
        }
        if (hasSession()) api.sessionNext().catch((e) => log(`next error: ${e}`));
        else api.next().catch((e) => log(`next error: ${e}`));
      },
    ],
    [
      "previoustrack",
      () => {
        log("action: prev");
        if (!hasOutput()) {
          log("no playback output for prev");
          return;
        }
        if (hasSession()) api.sessionPrev().catch((e) => log(`prev error: ${e}`));
        else api.prev().catch((e) => log(`prev error: ${e}`));
      },
    ],
    [
      "seekforward",
      () => {
        log("action: seekforward");
        if (hasOutput()) api.seekForward().catch((e) => log(`seekfwd error: ${e}`));
      },
    ],
    [
      "seekbackward",
      () => {
        log("action: seekbackward");
        if (hasOutput()) api.seekBackward().catch((e) => log(`seekback error: ${e}`));
      },
    ],
  ];
}

export function useMediaSession() {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const lastVolumeRef = useRef(VOLUME_MIDPOINT);

  const logInit = useCallback(() => {
    log(`mediaSession in navigator: ${"mediaSession" in navigator}`);
    log(`userAgent: ${navigator.userAgent.slice(0, 80)}`);
  }, []);

  // Log on mount
  useEffect(() => {
    logInit();
  }, [logInit]);

  useAudioActivation(audioRef, lastVolumeRef);
  useVisibilityReclaim(audioRef);
  useMetadataSync();
  useActionHandlers();
}
