import { useState, useCallback } from "react";
import { usePlayerStore } from "../../stores/playerStore";
import { selectPlaybackDevice, useDeviceStore } from "../../stores/deviceStore";
import { api } from "../../api/client";

interface Props {
  onExpand: () => void;
}

export function MiniPlayer({ onExpand }: Props) {
  const { playing, currentTrack, elapsedSeconds, durationSeconds, allowedActions } =
    usePlayerStore();
  const session = usePlayerStore((s) => s.session);
  const hasOutput = useDeviceStore((state) => selectPlaybackDevice(state.devices) != null);

  const progress = durationSeconds > 0 ? (elapsedSeconds / durationSeconds) * 100 : 0;
  const hasTrack = !!currentTrack;

  const handlePlayPause = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!hasOutput) return;
      if (playing) await api.pause();
      else await api.resume();
    },
    [hasOutput, playing]
  );

  const handleNext = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!hasOutput) return;
      if (session) await api.sessionNext();
      else await api.next();
    },
    [hasOutput, session]
  );

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onExpand}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onExpand();
      }}
      className="mini-player bg-[var(--color-surface-elevated)] border-t border-white/10 cursor-pointer md:hidden"
    >
      <MiniProgressBar hasTrack={hasTrack} durationSeconds={durationSeconds} progress={progress} />

      <div className="flex items-center gap-3 px-3 py-2">
        <MiniArt
          trackId={hasTrack ? currentTrack.id : null}
          fallbackChar={hasTrack ? (currentTrack.title?.[0] ?? "?") : "\u266A"}
        />
        <MiniTrackInfo title={currentTrack?.title} artist={currentTrack?.artist} />
        <button
          onClick={handlePlayPause}
          className="w-9 h-9 flex items-center justify-center text-[var(--color-text-primary)] shrink-0"
        >
          {playing ? <PauseIcon /> : <PlayIcon />}
        </button>
        <button
          onClick={handleNext}
          disabled={allowedActions.length > 0 && !allowedActions.includes("Next")}
          className="w-8 h-8 flex items-center justify-center text-[var(--color-text-secondary)] shrink-0 disabled:opacity-30"
        >
          <NextIcon />
        </button>
      </div>
    </div>
  );
}

function MiniProgressBar({
  hasTrack,
  durationSeconds,
  progress,
}: {
  hasTrack: boolean;
  durationSeconds: number;
  progress: number;
}) {
  return (
    <div className="h-0.5 bg-white/5">
      {hasTrack && durationSeconds > 0 && (
        <div
          className="h-full bg-[var(--color-accent)] transition-all duration-1000 ease-linear mini-progress-bar"
          ref={(el) => {
            if (el) el.style.setProperty("--progress", `${progress}%`);
          }}
        />
      )}
    </div>
  );
}

function MiniTrackInfo({ title, artist }: { title?: string | null; artist?: string | null }) {
  return (
    <div className="flex-1 min-w-0">
      <div className="text-sm font-medium truncate">{title ?? "Not playing"}</div>
      <div className="text-xs text-[var(--color-text-secondary)] truncate">
        {artist ?? "\u00A0"}
      </div>
    </div>
  );
}

function MiniArt({ trackId, fallbackChar }: { trackId: string | null; fallbackChar: string }) {
  const [failed, setFailed] = useState(false);
  if (trackId && !failed) {
    return (
      <img
        src={api.artUrl(trackId)}
        alt=""
        loading="lazy"
        onError={() => setFailed(true)}
        className="w-10 h-10 rounded-lg object-cover shrink-0"
      />
    );
  }
  return (
    <div className="w-10 h-10 rounded-lg bg-[var(--color-surface-hover)] flex items-center justify-center text-[var(--color-text-secondary)] text-lg shrink-0">
      {fallbackChar}
    </div>
  );
}

function PlayIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
      <polygon points="5,3 19,12 5,21" />
    </svg>
  );
}

function PauseIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
      <rect x="6" y="4" width="4" height="16" />
      <rect x="14" y="4" width="4" height="16" />
    </svg>
  );
}

function NextIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
      <polygon points="5,4 15,12 5,20" />
      <rect x="17" y="4" width="2" height="16" />
    </svg>
  );
}
