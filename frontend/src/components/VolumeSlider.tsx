import { useEffect, useRef, useState } from "react";

import { clampVolume } from "../hooks/useDeviceVolume";

interface VolumeSliderProps {
  value: number;
  label: string;
  ariaLabel: string;
  disabled?: boolean;
  onCommit: (volume: number) => Promise<unknown>;
}

export function VolumeSlider({
  value,
  label,
  ariaLabel,
  disabled = false,
  onCommit,
}: VolumeSliderProps) {
  const externalPercent = Math.round(clampVolume(value) * 100);
  const [draftPercent, setDraftPercent] = useState(externalPercent);
  const [pending, setPending] = useState(false);
  const dirtyRef = useRef(false);

  useEffect(() => {
    if (!dirtyRef.current && !pending) setDraftPercent(externalPercent);
  }, [externalPercent, pending]);

  const commit = async () => {
    if (!dirtyRef.current || pending || disabled) return;
    dirtyRef.current = false;
    setPending(true);
    try {
      await onCommit(draftPercent / 100);
    } catch {
      setDraftPercent(externalPercent);
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-white/50 w-12">{label}</span>
      <input
        type="range"
        min={0}
        max={100}
        value={draftPercent}
        disabled={disabled || pending}
        onChange={(event) => {
          dirtyRef.current = true;
          setDraftPercent(Number(event.target.value));
        }}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        aria-label={ariaLabel}
        className="seek-bar flex-1 disabled:opacity-50"
      />
      <span className="text-xs tabular-nums text-white/60 w-8 text-right">{draftPercent}</span>
    </div>
  );
}
