import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor, fireEvent } from "@testing-library/react";
import { renderWithProviders } from "../../test-utils";
import { EQSettings } from "./EQSettings";
import { useDeviceStore } from "../../stores/deviceStore";
import type { Device } from "../../api/client";

const { mockSwitchSource } = vi.hoisted(() => ({
  mockSwitchSource: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../api/client", () => ({
  api: {
    getEqState: vi.fn(() =>
      Promise.resolve({
        enabled: true,
        preset_name: "Rock",
        bands: [],
        channel_mode: "Stereo",
        source_name: "wifi",
      })
    ),
    getEqPresets: vi.fn(() =>
      Promise.resolve({ presets: ["Flat", "Rock", "Pop", "Jazz", "Classical"] })
    ),
    getBalance: vi.fn(() => Promise.resolve({ balance: 0 })),
    getCrossfade: vi.fn(() => Promise.resolve({ enabled: true })),
    getWifiStatus: vi.fn(() => Promise.resolve({ source: "wifi", rssi: -55, ssid: "HomeNet" })),
    switchSource: mockSwitchSource,
    setBalance: vi.fn(() => Promise.resolve()),
    setCrossfade: vi.fn(() => Promise.resolve()),
    loadEqPreset: vi.fn(() => Promise.resolve({})),
    enableEq: vi.fn(() => Promise.resolve()),
    disableEq: vi.fn(() => Promise.resolve()),
  },
}));

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

beforeEach(() => {
  useDeviceStore.setState({ devices: [], settingsDeviceId: null });
  vi.restoreAllMocks();
});

describe("EQSettings basic rendering", () => {
  it("shows the empty WiiM state when no settings device exists", () => {
    renderWithProviders(<EQSettings />);
    expect(screen.getByText("No WiiM speakers found")).toBeInTheDocument();
  });

  it("shows device info when device is selected", () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a", name: "Kitchen", ip: `10.0.0.${5}` })],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    expect(screen.getByText("Kitchen")).toBeInTheDocument();
  });

  it("shows EQ unavailable message when https_api is false", () => {
    useDeviceStore.setState({
      devices: [
        makeDevice({
          id: "a",
          capabilities: {
            av_transport: true,
            rendering_control: true,
            wiim_extended: true,
            https_api: false,
          },
        }),
      ],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    expect(screen.getByText("EQ Unavailable")).toBeInTheDocument();
  });

  it("renders three tabs when https_api is available", () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a" })],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    expect(screen.getByText("Presets")).toBeInTheDocument();
    expect(screen.getByText("EQ Bands")).toBeInTheDocument();
    expect(screen.getByText("Audio")).toBeInTheDocument();
  });
});

describe("EQSettings tabs and interactions", () => {
  it("loads and displays EQ presets", async () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a" })],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    await waitFor(() => {
      expect(screen.getByText("Flat")).toBeInTheDocument();
      expect(screen.getAllByText("Rock").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("Jazz")).toBeInTheDocument();
    });
  });

  it("shows input source buttons on Audio tab", async () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a" })],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    fireEvent.click(screen.getByText("Audio"));
    await waitFor(() => {
      expect(screen.getByText("Input Source")).toBeInTheDocument();
      expect(screen.getByText("WiFi")).toBeInTheDocument();
      expect(screen.getByText("Bluetooth")).toBeInTheDocument();
      expect(screen.getByText("Optical")).toBeInTheDocument();
      expect(screen.getByText("Line In")).toBeInTheDocument();
    });
  });

  it("calls switchSource when a source button is clicked", async () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a" })],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    fireEvent.click(screen.getByText("Audio"));
    await waitFor(() => {
      expect(screen.getByText("Bluetooth")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("Bluetooth"));
    await waitFor(() => {
      expect(mockSwitchSource).toHaveBeenCalledWith("a", "bluetooth");
    });
  });

  it("shows WiFi signal strength and SSID on Audio tab", async () => {
    useDeviceStore.setState({
      devices: [makeDevice({ id: "a" })],
      settingsDeviceId: "a",
    });
    renderWithProviders(<EQSettings />);
    fireEvent.click(screen.getByText("Audio"));
    await waitFor(() => {
      expect(screen.getByText("WiFi Signal")).toBeInTheDocument();
      expect(screen.getByText(/HomeNet/)).toBeInTheDocument();
      expect(screen.getByText(/-55 dBm/)).toBeInTheDocument();
      expect(screen.getByText("Good")).toBeInTheDocument();
    });
  });
});
