import { useState, useEffect } from "react";
import { api } from "../../api/client";
import { usePlayerStore } from "../../stores/playerStore";

export function useSleepTimerSync() {
  const setSleepRemaining = usePlayerStore((s) => s.setSleepRemaining);
  const sleepRemaining = usePlayerStore((s) => s.sleepRemaining);

  // Fetch the one global sleep timer on mount.
  useEffect(() => {
    let cancelled = false;
    api
      .getSleepTimer()
      .then((res) => {
        if (!cancelled) setSleepRemaining(res.remaining_seconds);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [setSleepRemaining]);

  // Local countdown
  const hasActiveSleep = sleepRemaining != null && sleepRemaining > 0;
  useEffect(() => {
    if (!hasActiveSleep) return;
    const interval = setInterval(() => {
      usePlayerStore.setState((s) => ({
        sleepRemaining:
          s.sleepRemaining != null && s.sleepRemaining > 1
            ? s.sleepRemaining - 1
            : s.sleepRemaining,
      }));
    }, 1000);
    return () => clearInterval(interval);
  }, [hasActiveSleep]);
}

export function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function NowPlayingArt({
  trackId,
  title,
}: {
  trackId: string | null;
  title: string | null;
}) {
  const [failed, setFailed] = useState(false);
  const [lastTrackId, setLastTrackId] = useState(trackId);
  if (trackId !== lastTrackId) {
    setLastTrackId(trackId);
    setFailed(false);
  }

  if (trackId && !failed) {
    return (
      <img
        src={api.artUrl(trackId)}
        alt=""
        onError={() => setFailed(true)}
        className="w-full max-w-[320px] aspect-square rounded-2xl object-cover shadow-2xl"
      />
    );
  }
  return (
    <div className="w-full max-w-[320px] aspect-square rounded-2xl bg-white/5 flex items-center justify-center">
      <div className="text-6xl text-white/30">{title?.[0]?.toUpperCase() ?? "\u266A"}</div>
    </div>
  );
}
