import { useCallback } from "react";
import { api, type Device } from "../api/client";
import { selectPlaybackDevice, useDeviceStore } from "../stores/deviceStore";

export function clampVolume(volume: number): number {
  return Math.max(0, Math.min(1, volume));
}

export function volumePercent(device: Device): number {
  return Math.round(clampVolume(device.volume) * 100);
}

export function setPlaybackVolume(volume: number) {
  const next = clampVolume(volume);
  const store = useDeviceStore.getState();
  for (const output of store.devices.filter((device) => device.enabled)) {
    store.updateDevice(output.id, { volume: next });
  }
  return api.setPlaybackVolume(next);
}

export function usePlaybackVolumeActions() {
  const setVolume = useCallback((volume: number) => setPlaybackVolume(volume), []);
  const adjustVolume = useCallback(async (delta: number) => {
    const output = selectPlaybackDevice(useDeviceStore.getState().devices);
    if (output) await setPlaybackVolume(output.volume + delta);
  }, []);

  return { setVolume, adjustVolume };
}
