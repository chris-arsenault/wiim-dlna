import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Device } from "../../api/client";
import { useDeviceStore } from "../../stores/deviceStore";
import { DeviceManager } from "./DeviceManager";

const { mockSetEnabled } = vi.hoisted(() => ({
  mockSetEnabled: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../api/client", () => ({
  api: { setEnabled: mockSetEnabled },
}));

function makeDevice(overrides: Partial<Device> = {}): Device {
  return {
    id: "dev-1",
    name: "Living Room",
    ip: `192.168.1.${10}`,
    model: "WiiM Pro",
    firmware: "4.8.1",
    device_type: "wiim",
    enabled: true,
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

beforeEach(() => {
  vi.clearAllMocks();
  useDeviceStore.setState({ devices: [], settingsDeviceId: null });
});

describe("DeviceManager", () => {
  it("shows WiiM discovery state when no speakers are known", () => {
    render(<DeviceManager />);
    expect(screen.getByText("Discovering WiiM speakers...")).toBeInTheDocument();
  });

  it("renders each speaker as an on/off output", () => {
    useDeviceStore.setState({
      devices: [
        makeDevice({ id: "a", name: "Kitchen" }),
        makeDevice({ id: "b", name: "Bedroom", enabled: false }),
      ],
    });
    render(<DeviceManager />);
    expect(screen.getByRole("switch", { name: "Kitchen output" })).toHaveAttribute(
      "aria-checked",
      "true"
    );
    expect(screen.getByRole("switch", { name: "Bedroom output" })).toHaveAttribute(
      "aria-checked",
      "false"
    );
  });

  it("turns a speaker off through output membership and updates after success", async () => {
    useDeviceStore.setState({ devices: [makeDevice({ id: "a", name: "Kitchen" })] });
    render(<DeviceManager />);
    fireEvent.click(screen.getByRole("switch", { name: "Kitchen output" }));
    await waitFor(() => expect(mockSetEnabled).toHaveBeenCalledWith("a", false));
    expect(useDeviceStore.getState().devices[0].enabled).toBe(false);
  });

  it("does not expose grouping, presets, mute, or volume controls", () => {
    useDeviceStore.setState({ devices: [makeDevice()] });
    render(<DeviceManager />);
    expect(screen.queryByText("Group")).toBeNull();
    expect(screen.queryByText("Presets")).toBeNull();
    expect(screen.queryByTitle("Mute")).toBeNull();
    expect(screen.queryByRole("slider")).toBeNull();
  });
});
