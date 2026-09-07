import { create } from "zustand";
import type { QueueTrack, SessionInfo } from "../api/client";

interface PlayerState {
  playing: boolean;
  volume: number;
  currentTrack: QueueTrack | null;
  elapsedSeconds: number;
  durationSeconds: number;
  shuffleMode: string;
  repeatMode: string;
  session: SessionInfo | null;
  allowedActions: string[];
  rating: number;
  sleepRemaining: number | null;
  setPlaying: (playing: boolean) => void;
  setVolume: (volume: number) => void;
  setCurrentTrack: (track: QueueTrack | null) => void;
  setElapsed: (seconds: number) => void;
  setDuration: (seconds: number) => void;
  setShuffleMode: (mode: string) => void;
  setRepeatMode: (mode: string) => void;
  setSession: (session: SessionInfo | null) => void;
  setAllowedActions: (actions: string[]) => void;
  setRating: (rating: number) => void;
  setSleepRemaining: (remaining: number | null) => void;
}

export const usePlayerStore = create<PlayerState>((set) => ({
  playing: false,
  volume: 1,
  currentTrack: null,
  elapsedSeconds: 0,
  durationSeconds: 0,
  shuffleMode: "off",
  repeatMode: "off",
  session: null,
  allowedActions: [],
  rating: 0,
  sleepRemaining: null,
  setPlaying: (playing) => set({ playing }),
  setVolume: (volume) => set({ volume }),
  setCurrentTrack: (track) => set({ currentTrack: track, rating: 0 }),
  setElapsed: (seconds) => set({ elapsedSeconds: seconds }),
  setDuration: (seconds) => set({ durationSeconds: seconds }),
  setShuffleMode: (mode) => set({ shuffleMode: mode }),
  setRepeatMode: (mode) => set({ repeatMode: mode }),
  setSession: (session) => set({ session }),
  setAllowedActions: (actions) => set({ allowedActions: actions }),
  setRating: (rating) => set({ rating }),
  setSleepRemaining: (remaining) => set({ sleepRemaining: remaining }),
}));
