import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MiniPlayer } from "./MiniPlayer";
import { usePlayerStore } from "../../stores/playerStore";
import { useDeviceStore } from "../../stores/deviceStore";

vi.mock("../../api/client", () => ({
  api: {
    pause: vi.fn(() => Promise.resolve()),
    resume: vi.fn(() => Promise.resolve()),
    setVolume: vi.fn(() => Promise.resolve()),
    artUrl: vi.fn((id: string) => `/api/art/${id}`),
  },
}));

describe("MiniPlayer", () => {
  beforeEach(() => {
    usePlayerStore.setState({
      playing: false,
      currentTrack: null,
      elapsedSeconds: 0,
      durationSeconds: 0,
    });
    useDeviceStore.setState({
      devices: [
        {
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
        },
      ],
      settingsDeviceId: "dev-1",
    });
  });

  it("shows idle state when no track is playing", () => {
    render(<MiniPlayer onExpand={vi.fn()} />);
    expect(screen.getByText("Not playing")).toBeInTheDocument();
  });

  it("shows track info when a track is set", () => {
    usePlayerStore.setState({
      currentTrack: {
        id: "t1",
        title: "Test Song",
        artist: "Test Artist",
        album: null,
        duration: null,
        stream_url: null,
      },
      elapsedSeconds: 0,
      durationSeconds: 0,
    });
    render(<MiniPlayer onExpand={vi.fn()} />);
    expect(screen.getByText("Test Song")).toBeInTheDocument();
    expect(screen.getByText("Test Artist")).toBeInTheDocument();
  });

  it("calls onExpand when clicked", () => {
    usePlayerStore.setState({
      currentTrack: {
        id: "t1",
        title: "Song",
        artist: null,
        album: null,
        duration: null,
        stream_url: null,
      },
    });
    const onExpand = vi.fn();
    render(<MiniPlayer onExpand={onExpand} />);
    fireEvent.click(screen.getByText("Song"));
    expect(onExpand).toHaveBeenCalled();
  });
});
