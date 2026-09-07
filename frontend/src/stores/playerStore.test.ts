import { describe, it, expect, beforeEach } from "vitest";
import { usePlayerStore } from "./playerStore";

describe("playerStore", () => {
  beforeEach(() => {
    usePlayerStore.setState({
      playing: false,
      volume: 1,
      currentTrack: null,
      elapsedSeconds: 0,
      durationSeconds: 0,
      shuffleMode: "off",
      repeatMode: "off",
    });
  });

  it("starts with default state", () => {
    const state = usePlayerStore.getState();
    expect(state.playing).toBe(false);
    expect(state.volume).toBe(1);
    expect(state.currentTrack).toBeNull();
    expect(state.elapsedSeconds).toBe(0);
    expect(state.durationSeconds).toBe(0);
    expect(state.shuffleMode).toBe("off");
    expect(state.repeatMode).toBe("off");
  });

  it("stores the global volume multiplier", () => {
    usePlayerStore.getState().setVolume(0.65);
    expect(usePlayerStore.getState().volume).toBe(0.65);
  });

  it("setPlaying toggles playing state", () => {
    usePlayerStore.getState().setPlaying(true);
    expect(usePlayerStore.getState().playing).toBe(true);
    usePlayerStore.getState().setPlaying(false);
    expect(usePlayerStore.getState().playing).toBe(false);
  });

  it("setCurrentTrack stores track info", () => {
    const track = {
      id: "t1",
      title: "Song A",
      artist: "Artist A",
      album: "Album A",
      duration: "3:45",
      stream_url: "http://example.com/t1.flac",
    };
    usePlayerStore.getState().setCurrentTrack(track);
    expect(usePlayerStore.getState().currentTrack).toEqual(track);
  });

  it("setCurrentTrack can clear track", () => {
    usePlayerStore.getState().setCurrentTrack({
      id: "t1",
      title: "X",
      artist: null,
      album: null,
      duration: null,
      stream_url: null,
    });
    usePlayerStore.getState().setCurrentTrack(null);
    expect(usePlayerStore.getState().currentTrack).toBeNull();
  });

  it("setElapsed and setDuration update time", () => {
    usePlayerStore.getState().setElapsed(45);
    usePlayerStore.getState().setDuration(200);
    expect(usePlayerStore.getState().elapsedSeconds).toBe(45);
    expect(usePlayerStore.getState().durationSeconds).toBe(200);
  });

  it("setShuffleMode and setRepeatMode update modes", () => {
    usePlayerStore.getState().setShuffleMode("shuffle_all");
    usePlayerStore.getState().setRepeatMode("track");
    expect(usePlayerStore.getState().shuffleMode).toBe("shuffle_all");
    expect(usePlayerStore.getState().repeatMode).toBe("track");
  });
});
