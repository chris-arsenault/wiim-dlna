import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Device } from "../api/client";
import { useDeviceStore } from "../stores/deviceStore";
import { usePlayerStore } from "../stores/playerStore";
import { setDeviceVolume, setPlaybackVolume } from "./useDeviceVolume";

const { mockSetPlaybackVolume, mockSetVolume } = vi.hoisted(() => ({
  mockSetPlaybackVolume: vi.fn(() => Promise.resolve()),
  mockSetVolume: vi.fn(() => Promise.resolve()),
}));

vi.mock("../api/client", () => ({
  api: {
    setPlaybackVolume: mockSetPlaybackVolume,
    setVolume: mockSetVolume,
  },
}));

function makeDevice(id: string, volume: number): Device {
  return {
    id,
    name: id,
    ip: "speaker.local",
    model: "WiiM Pro",
    firmware: "4.8.1",
    device_type: "wiim",
    enabled: true,
    output_target: null,
    output_error: null,
    capabilities: {
      av_transport: true,
      rendering_control: true,
      wiim_extended: true,
      https_api: true,
    },
    volume,
    muted: false,
    channel: null,
    source: "wifi",
    group_id: null,
    is_master: false,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  usePlayerStore.setState({ volume: 1 });
  useDeviceStore.setState({
    devices: [makeDevice("kitchen", 0.8), makeDevice("bedroom", 0.3)],
    settingsDeviceId: null,
  });
});

describe("volume actions", () => {
  it("changes the global multiplier without flattening speaker base levels", async () => {
    await setPlaybackVolume(0.5);

    expect(usePlayerStore.getState().volume).toBe(0.5);
    expect(useDeviceStore.getState().devices.map((device) => device.volume)).toEqual([0.8, 0.3]);
    expect(mockSetPlaybackVolume).toHaveBeenCalledWith(0.5);
  });

  it("changes only the selected speaker base level", async () => {
    await setDeviceVolume("bedroom", 0.6);

    expect(useDeviceStore.getState().devices.map((device) => device.volume)).toEqual([0.8, 0.6]);
    expect(mockSetVolume).toHaveBeenCalledWith("bedroom", 0.6);
  });

  it("rolls an optimistic global change back after a failed write", async () => {
    mockSetPlaybackVolume.mockRejectedValueOnce(new Error("write failed"));

    await expect(setPlaybackVolume(0.4)).rejects.toThrow("write failed");
    expect(usePlayerStore.getState().volume).toBe(1);
  });
});
