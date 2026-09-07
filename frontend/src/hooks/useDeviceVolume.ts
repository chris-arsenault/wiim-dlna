import { useCallback } from "react";
import { api } from "../api/client";
import { useDeviceStore } from "../stores/deviceStore";
import { usePlayerStore } from "../stores/playerStore";

export function clampVolume(volume: number): number {
  return Math.max(0, Math.min(1, volume));
}

export function setPlaybackVolume(volume: number) {
  const next = clampVolume(volume);
  const previous = usePlayerStore.getState().volume;
  usePlayerStore.setState({ volume: next });
  return api.setPlaybackVolume(next).catch((error) => {
    if (usePlayerStore.getState().volume === next) {
      usePlayerStore.setState({ volume: previous });
    }
    throw error;
  });
}

export function setDeviceVolume(deviceId: string, volume: number) {
  const next = clampVolume(volume);
  const store = useDeviceStore.getState();
  const previous = store.devices.find((device) => device.id === deviceId)?.volume;
  store.updateDevice(deviceId, { volume: next });
  return api.setVolume(deviceId, next).catch((error) => {
    const current = useDeviceStore
      .getState()
      .devices.find((device) => device.id === deviceId)?.volume;
    if (previous != null && current === next) {
      useDeviceStore.getState().updateDevice(deviceId, { volume: previous });
    }
    throw error;
  });
}

export function usePlaybackVolumeActions() {
  const setVolume = useCallback((volume: number) => setPlaybackVolume(volume), []);
  const adjustVolume = useCallback(async (delta: number) => {
    await setPlaybackVolume(usePlayerStore.getState().volume + delta);
  }, []);

  return { setVolume, adjustVolume };
}
