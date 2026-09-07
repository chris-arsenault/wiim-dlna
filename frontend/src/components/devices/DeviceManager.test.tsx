import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Device } from "../../api/client";
import { useDeviceStore } from "../../stores/deviceStore";
import { DeviceManager } from "./DeviceManager";

const { mockSetEnabled, mockSetVolume, mockRecoverOutputs } = vi.hoisted(() => ({
  mockSetEnabled: vi.fn(() => Promise.resolve()),
  mockSetVolume: vi.fn(() => Promise.resolve()),
  mockRecoverOutputs: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../api/client", () => ({
  api: {
    setEnabled: mockSetEnabled,
    setVolume: mockSetVolume,
    recoverOutputs: mockRecoverOutputs,
  },
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

beforeEach(() => {
  vi.clearAllMocks();
  useDeviceStore.setState({
    devices: [],
    outputRecovery: { required: false, in_progress: false, error: null },
    settingsDeviceId: null,
  });
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

  it("persists the desired state while the physical transition runs", async () => {
    useDeviceStore.setState({ devices: [makeDevice({ id: "a", name: "Kitchen" })] });
    render(<DeviceManager />);
    fireEvent.click(screen.getByRole("switch", { name: "Kitchen output" }));
    await waitFor(() => expect(mockSetEnabled).toHaveBeenCalledWith("a", false));
    expect(useDeviceStore.getState().devices[0]).toMatchObject({
      enabled: false,
      output_target: false,
    });
  });

  it("disables every toggle while WiiM hardware is converging", () => {
    useDeviceStore.setState({
      devices: [
        makeDevice({ id: "a", name: "Kitchen", output_target: false }),
        makeDevice({ id: "b", name: "Bedroom" }),
      ],
    });
    render(<DeviceManager />);

    expect(screen.getByText("Turning off…")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Kitchen output" })).toBeDisabled();
    expect(screen.getByRole("switch", { name: "Bedroom output" })).toBeDisabled();
    expect(screen.getByRole("slider", { name: "Kitchen volume" })).toBeDisabled();
    expect(screen.getByRole("slider", { name: "Bedroom volume" })).toBeDisabled();
  });

  it("shows a bounded transition failure returned by the server", () => {
    useDeviceStore.setState({
      devices: [makeDevice({ output_error: "timed out waiting for WiiM hardware to detach" })],
    });
    render(<DeviceManager />);
    expect(screen.getByText("timed out waiting for WiiM hardware to detach")).toBeInTheDocument();
  });
});

describe("DeviceManager recovery and volume", () => {
  it("edits desired membership without starting hardware work after recovery fails", async () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a", name: "Kitchen" })],
      outputRecovery: {
        required: true,
        in_progress: false,
        error: "bounded recovery failed",
      },
    });
    render(<DeviceManager />);

    fireEvent.click(screen.getByRole("switch", { name: "Kitchen output" }));
    await waitFor(() => expect(mockSetEnabled).toHaveBeenCalledWith("a", false));
    expect(useDeviceStore.getState().devices[0]).toMatchObject({
      enabled: false,
      output_target: null,
    });
    expect(screen.getByText("Wanted off")).toBeInTheDocument();
  });

  it("starts exactly one explicit recovery request", async () => {
    useDeviceStore.setState({
      devices: [makeDevice()],
      outputRecovery: {
        required: true,
        in_progress: false,
        error: "bounded recovery failed",
      },
    });
    render(<DeviceManager />);

    fireEvent.click(screen.getByRole("button", { name: "Recover speakers" }));
    await waitFor(() => expect(mockRecoverOutputs).toHaveBeenCalledTimes(1));
    expect(useDeviceStore.getState().outputRecovery.in_progress).toBe(true);
  });

  it("exposes per-speaker volume without restoring unrelated device controls", () => {
    useDeviceStore.setState({ devices: [makeDevice()] });
    render(<DeviceManager />);
    expect(screen.queryByText("Group")).toBeNull();
    expect(screen.queryByText("Presets")).toBeNull();
    expect(screen.queryByTitle("Mute")).toBeNull();
    expect(screen.getByRole<HTMLInputElement>("slider", { name: "Living Room volume" }).value).toBe(
      "50"
    );
  });

  it("commits one speaker-volume write after a slider interaction", async () => {
    useDeviceStore.setState({ devices: [makeDevice()] });
    render(<DeviceManager />);
    const slider = screen.getByRole("slider", { name: "Living Room volume" });
    fireEvent.change(slider, { target: { value: "72" } });
    expect(mockSetVolume).not.toHaveBeenCalled();
    fireEvent.pointerUp(slider);
    await waitFor(() => expect(mockSetVolume).toHaveBeenCalledWith("dev-1", 0.72));
    expect(mockSetVolume).toHaveBeenCalledTimes(1);
  });
});
