import { useCallback } from "react";
import { api } from "../api/client";
import { selectPlaybackDevice, useDeviceStore } from "../stores/deviceStore";
import { usePlayerStore } from "../stores/playerStore";

interface PlayOptions {
  shuffle?: boolean;
  startTrackId?: string;
}

/** Plays a saved playlist on the shared Airwave stream, optionally shuffled. */
export function usePlaylistPlayback(onPlay?: () => void) {
  const canPlay = useDeviceStore((state) => selectPlaybackDevice(state.devices) != null);
  const setPlaying = usePlayerStore((s) => s.setPlaying);

  const play = useCallback(
    async (playlistId: number, options: PlayOptions = {}) => {
      if (!canPlay) return;
      await api.sessionPlay({
        source_id: api.playlistSourceId(playlistId),
        start_track_id: options.startTrackId,
        shuffle: options.shuffle ? "tracks" : undefined,
      });
      setPlaying(true);
      onPlay?.();
    },
    [canPlay, setPlaying, onPlay]
  );

  return { canPlay, play };
}
