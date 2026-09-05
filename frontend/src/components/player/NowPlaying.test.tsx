import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, fireEvent } from "@testing-library/react";
import { renderWithProviders } from "../../test-utils";
import { NowPlaying } from "./NowPlaying";
import { usePlayerStore } from "../../stores/playerStore";
import { useDeviceStore } from "../../stores/deviceStore";
import type { Device } from "../../api/client";

const { mockPause, mockResume, mockSetVolume } = vi.hoisted(() => ({
  mockPause: vi.fn(() => Promise.resolve()),
  mockResume: vi.fn(() => Promise.resolve()),
  mockSetVolume: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../api/client", () => ({
  api: {
    pause: mockPause,
    resume: mockResume,
    next: vi.fn(() => Promise.resolve()),
    prev: vi.fn(() => Promise.resolve()),
    seek: vi.fn(() => Promise.resolve()),
    seekForward: vi.fn(() => Promise.resolve()),
    seekBackward: vi.fn(() => Promise.resolve()),
    setPlaybackVolume: mockSetVolume,
    setShuffle: vi.fn(() => Promise.resolve()),
    setRepeat: vi.fn(() => Promise.resolve()),
    rateTrack: vi.fn(() => Promise.resolve()),
    getSleepTimer: vi.fn(() => Promise.resolve({ remaining_seconds: null })),
    setSleepTimer: vi.fn(() => Promise.resolve()),
    cancelSleepTimer: vi.fn(() => Promise.resolve()),
    artUrl: vi.fn((id: string) => `/api/art/${id}`),
  },
}));

vi.mock("../../hooks/useArtColor", () => ({
  useArtColor: () => ({ dominant: "#6366f1", muted: "#2d2b55" }),
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

describe("NowPlaying", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePlayerStore.setState({
      playing: false,
      currentTrack: {
        id: "t1",
        title: "Test Song",
        artist: "Test Artist",
        album: "Test Album",
        duration: "4:30",
        stream_url: null,
      },
      elapsedSeconds: 60,
      durationSeconds: 270,
      shuffleMode: "off",
      repeatMode: "off",
    });
    useDeviceStore.setState({
      devices: [makeDevice()],
      settingsDeviceId: "dev-1",
    });
  });

  it("shows track info", () => {
    renderWithProviders(<NowPlaying />);
    expect(screen.getByText("Test Song")).toBeInTheDocument();
    expect(screen.getByText("Test Artist \u2014 Test Album")).toBeInTheDocument();
  });

  it("shows the current output name without a selector", () => {
    renderWithProviders(<NowPlaying />);
    expect(screen.getByText("Living Room")).toBeInTheDocument();
  });

  it("shows the playing group volume slider", () => {
    renderWithProviders(<NowPlaying />);
    expect(
      screen.getByRole<HTMLInputElement>("slider", { name: "Playing group volume" }).value
    ).toBe("50");
  });

  it("shows formatted time", () => {
    renderWithProviders(<NowPlaying />);
    expect(screen.getByText("1:00")).toBeInTheDocument(); // 60s
    expect(screen.getByText("4:30")).toBeInTheDocument(); // 270s
  });

  it('shows "Nothing playing" when no track', () => {
    usePlayerStore.setState({ currentTrack: null });
    renderWithProviders(<NowPlaying />);
    expect(screen.getByText("Nothing playing")).toBeInTheDocument();
  });

  it("calls api.resume when play is clicked while paused", () => {
    usePlayerStore.setState({ playing: false });
    renderWithProviders(<NowPlaying />);
    const bigButton = document.querySelector("button.w-16")!;
    fireEvent.click(bigButton);
    expect(mockResume).toHaveBeenCalledWith();
  });

  it("calls api.pause when pause is clicked while playing", () => {
    usePlayerStore.setState({ playing: true });
    renderWithProviders(<NowPlaying />);
    const bigButton = document.querySelector("button.w-16")!;
    fireEvent.click(bigButton);
    expect(mockPause).toHaveBeenCalledWith();
  });

  it("sets the playing group volume from the main slider", () => {
    renderWithProviders(<NowPlaying />);
    fireEvent.change(screen.getByRole("slider", { name: "Playing group volume" }), {
      target: { value: "65" },
    });
    expect(mockSetVolume).toHaveBeenCalledWith(0.65);
  });
});
