import { create } from "zustand";
import type { Device, OutputRecoveryState } from "../api/client";

const IDLE_OUTPUT_RECOVERY: OutputRecoveryState = {
  required: false,
  in_progress: false,
  error: null,
};

interface DeviceState {
  devices: Device[];
  outputRecovery: OutputRecoveryState;
  settingsDeviceId: string | null;
  setDevices: (devices: Device[]) => void;
  setOutputRecovery: (state: OutputRecoveryState) => void;
  setSettingsDevice: (id: string) => void;
  updateDevice: (id: string, update: Partial<Device>) => void;
}

export function selectPlaybackDevice(devices: Device[]): Device | undefined {
  const enabled = devices
    .filter(
      (device) =>
        device.enabled && device.device_type === "wiim" && device.capabilities.av_transport
    )
    .sort((left, right) => left.id.localeCompare(right.id));
  return (
    enabled.find(
      (device) => device.is_master && device.group_id != null && device.group_id === device.id
    ) ??
    enabled.find((device) => device.group_id == null) ??
    enabled[0]
  );
}

export const useDeviceStore = create<DeviceState>((set) => ({
  devices: [],
  outputRecovery: IDLE_OUTPUT_RECOVERY,
  settingsDeviceId: null,
  setDevices: (devices) =>
    set((state) => {
      const wiimDevices = devices.filter((device) => device.device_type === "wiim");
      const selectedStillExists = wiimDevices.some(
        (device) => device.id === state.settingsDeviceId
      );
      return {
        devices: wiimDevices,
        settingsDeviceId: selectedStillExists
          ? state.settingsDeviceId
          : (wiimDevices[0]?.id ?? null),
      };
    }),
  setOutputRecovery: (outputRecovery) => set({ outputRecovery }),
  setSettingsDevice: (id) => set({ settingsDeviceId: id }),
  updateDevice: (id, update) =>
    set((state) => ({
      devices: state.devices.map((device) =>
        device.id === id ? { ...device, ...update } : device
      ),
    })),
}));
