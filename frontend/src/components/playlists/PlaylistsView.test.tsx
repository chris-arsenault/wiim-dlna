import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor, fireEvent } from "@testing-library/react";
import { renderWithProviders } from "../../test-utils";
import { PlaylistsView } from "./PlaylistsView";
import { useDeviceStore } from "../../stores/deviceStore";
import type { Device } from "../../api/client";

const TEST_DEVICE: Device = {
  id: "dev-1",
  name: "Living Room",
  ip: "192.0.2.10",
  model: "WiiM Mini",
  firmware: null,
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
  source: null,
  group_id: null,
  is_master: false,
};

const {
  mockGetPlaylists,
  mockGetPlaylist,
  mockCreatePlaylist,
  mockDeletePlaylist,
  mockRemoveFromPlaylist,
  mockSessionPlay,
} = vi.hoisted(() => ({
  mockGetPlaylists: vi.fn(),
  mockGetPlaylist: vi.fn(),
  mockCreatePlaylist: vi.fn(() => Promise.resolve({ id: 2, name: "New", track_count: 0 })),
  mockDeletePlaylist: vi.fn(() => Promise.resolve()),
  mockRemoveFromPlaylist: vi.fn(() => Promise.resolve()),
  mockSessionPlay: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../api/client", () => ({
  api: {
    getPlaylists: mockGetPlaylists,
    getPlaylist: mockGetPlaylist,
    createPlaylist: mockCreatePlaylist,
    deletePlaylist: mockDeletePlaylist,
    addToPlaylist: vi.fn(() => Promise.resolve({ added: 0 })),
    removeFromPlaylist: mockRemoveFromPlaylist,
    playlistSourceId: (id: number) => `pl${id}`,
    sessionPlay: mockSessionPlay,
  },
}));

const PLAYLIST = {
  id: 1,
  name: "Road Trip",
  track_count: 2,
  created_at: null,
  updated_at: null,
};

const DETAIL = {
  ...PLAYLIST,
  tracks: [
    { track_id: "t1", position: 0, title: "Track One", artist: "Artist", album: "Album" },
    { track_id: "t2", position: 1, title: "Track Two", artist: "Artist", album: "Album" },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  mockGetPlaylists.mockResolvedValue([PLAYLIST]);
  mockGetPlaylist.mockResolvedValue(DETAIL);
  useDeviceStore.setState({ devices: [TEST_DEVICE], settingsDeviceId: "dev-1" });
});

describe("PlaylistsView list", () => {
  it("shows playlists with track counts", async () => {
    renderWithProviders(<PlaylistsView />);
    expect(await screen.findByText("Road Trip")).toBeInTheDocument();
    expect(screen.getByText("2 tracks")).toBeInTheDocument();
  });

  it("creates a playlist", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.change(screen.getByPlaceholderText("New playlist name"), {
      target: { value: "Dinner" },
    });
    fireEvent.click(screen.getByText("Create"));
    await waitFor(() => expect(mockCreatePlaylist).toHaveBeenCalledWith("Dinner"));
  });

  it("plays a playlist as a session source", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.click(await screen.findByTitle("Play playlist"));
    await waitFor(() =>
      expect(mockSessionPlay).toHaveBeenCalledWith({
        source_id: "pl1",
        start_track_id: undefined,
        shuffle: undefined,
      })
    );
  });

  it("shuffles a playlist", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.click(await screen.findByTitle("Shuffle playlist"));
    await waitFor(() =>
      expect(mockSessionPlay).toHaveBeenCalledWith({
        source_id: "pl1",
        start_track_id: undefined,
        shuffle: "tracks",
      })
    );
  });

  it("deletes a playlist", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.click(await screen.findByTitle("Delete playlist"));
    await waitFor(() => expect(mockDeletePlaylist).toHaveBeenCalledWith(1));
  });
});

describe("PlaylistsView detail", () => {
  it("opens a playlist and lists its tracks", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.click(await screen.findByText("Road Trip"));
    expect(await screen.findByText("Track One")).toBeInTheDocument();
    expect(screen.getByText("Track Two")).toBeInTheDocument();
  });

  it("starts playback at the clicked track", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.click(await screen.findByText("Road Trip"));
    fireEvent.click(await screen.findByText("Track Two"));
    await waitFor(() =>
      expect(mockSessionPlay).toHaveBeenCalledWith({
        source_id: "pl1",
        start_track_id: "t2",
        shuffle: undefined,
      })
    );
  });

  it("removes a track by position", async () => {
    renderWithProviders(<PlaylistsView />);
    fireEvent.click(await screen.findByText("Road Trip"));
    const removeButtons = await screen.findAllByTitle("Remove from playlist");
    fireEvent.click(removeButtons[1]);
    await waitFor(() => expect(mockRemoveFromPlaylist).toHaveBeenCalledWith(1, 1));
  });
});
