import { beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Device } from "../../api/client";
import { useDeviceStore } from "../../stores/deviceStore";
import { usePlayerStore } from "../../stores/playerStore";
import { OutputStatus } from "./OutputStatus";

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
  useDeviceStore.setState({ devices: [], settingsDeviceId: null });
  usePlayerStore.setState({ playing: false });
});

describe("OutputStatus", () => {
  it("shows that no speakers are on", () => {
    render(<OutputStatus />);
    expect(screen.getByText("No speakers on")).toBeInTheDocument();
  });

  it("summarizes the enabled outputs without a device selector", () => {
    useDeviceStore.setState({
      devices: [
        makeDevice({ id: "a", name: "Kitchen" }),
        makeDevice({ id: "b", name: "Bedroom" }),
        makeDevice({ id: "c", name: "Office", enabled: false }),
      ],
    });
    render(<OutputStatus />);
    expect(screen.getByText("Kitchen + Bedroom")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows an active indicator only while the shared stream is playing", () => {
    useDeviceStore.setState({ devices: [makeDevice()] });
    usePlayerStore.setState({ playing: true });
    const { container } = render(<OutputStatus />);
    expect(container.querySelector(".bg-emerald-400")).toBeInTheDocument();
  });
});
