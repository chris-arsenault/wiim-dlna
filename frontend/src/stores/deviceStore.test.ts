import { beforeEach, describe, expect, it } from "vitest";
import type { Device } from "../api/client";
import { selectPlaybackDevice, useDeviceStore } from "./deviceStore";

const TEST_DEVICE_IP = `192.168.1.${10}`;

function makeDevice(overrides: Partial<Device> = {}): Device {
  return {
    id: "dev-1",
    name: "Living Room",
    ip: TEST_DEVICE_IP,
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
    volume: 0.5,
    muted: false,
    channel: null,
    source: "wifi",
    group_id: null,
    is_master: false,
    ...overrides,
  };
}

describe("deviceStore", () => {
  beforeEach(() => {
    useDeviceStore.setState({
      devices: [],
      outputRecovery: { required: false, in_progress: false, error: null },
      settingsDeviceId: null,
    });
  });

  it("starts with no discovered WiiM speakers", () => {
    const { devices, outputRecovery, settingsDeviceId } = useDeviceStore.getState();
    expect(devices).toEqual([]);
    expect(outputRecovery).toEqual({ required: false, in_progress: false, error: null });
    expect(settingsDeviceId).toBeNull();
  });

  it("filters non-WiiM renderers and selects the first WiiM for settings", () => {
    useDeviceStore
      .getState()
      .setDevices([makeDevice({ id: "tv", device_type: "renderer" }), makeDevice({ id: "wiim" })]);
    const state = useDeviceStore.getState();
    expect(state.devices.map((device) => device.id)).toEqual(["wiim"]);
    expect(state.settingsDeviceId).toBe("wiim");
  });

  it("keeps the explicit settings device while it remains available", () => {
    useDeviceStore.getState().setDevices([makeDevice({ id: "a" }), makeDevice({ id: "b" })]);
    useDeviceStore.getState().setSettingsDevice("b");
    useDeviceStore.getState().setDevices([makeDevice({ id: "a" }), makeDevice({ id: "b" })]);
    expect(useDeviceStore.getState().settingsDeviceId).toBe("b");
  });

  it("derives playback from the enabled physical master", () => {
    const devices = [
      makeDevice({ id: "a", group_id: "a", is_master: true }),
      makeDevice({ id: "b", group_id: "a" }),
    ];
    expect(selectPlaybackDevice(devices)?.id).toBe("a");
  });

  it("does not derive playback from disabled speakers", () => {
    expect(selectPlaybackDevice([makeDevice({ enabled: false })])).toBeUndefined();
  });

  it("does not derive playback from a speaker without AVTransport", () => {
    expect(
      selectPlaybackDevice([
        makeDevice({ capabilities: { ...makeDevice().capabilities, av_transport: false } }),
      ])
    ).toBeUndefined();
  });

  it("merges physical device updates without affecting other devices", () => {
    useDeviceStore
      .getState()
      .setDevices([makeDevice({ id: "a", volume: 0.5 }), makeDevice({ id: "b", volume: 0.3 })]);
    useDeviceStore.getState().updateDevice("a", { volume: 0.9, muted: true });
    expect(useDeviceStore.getState().devices[0].volume).toBe(0.9);
    expect(useDeviceStore.getState().devices[0].muted).toBe(true);
    expect(useDeviceStore.getState().devices[1].volume).toBe(0.3);
  });
});
