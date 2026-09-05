import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor, fireEvent } from "@testing-library/react";
import { renderWithProviders } from "../../test-utils";
import { LibraryBrowser } from "./LibraryBrowser";
import { useDeviceStore } from "../../stores/deviceStore";
import { useUiStore } from "../../stores/uiStore";

const TEST_DEVICE = {
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
} as const;

const { mockBrowse, mockSearch, mockSessionPlay, mockGetLibraryState, mockSetLibraryState } =
  vi.hoisted(() => ({
    mockBrowse: vi.fn(),
    mockSearch: vi.fn(),
    mockSessionPlay: vi.fn(() => Promise.resolve()),
    mockGetLibraryState: vi.fn(() => Promise.resolve({ path: [{ id: "0", title: "Library" }] })),
    mockSetLibraryState: vi.fn(() => Promise.resolve()),
  }));

vi.mock("../../api/client", () => ({
  api: {
    browse: mockBrowse,
    search: mockSearch,
    play: vi.fn(() => Promise.resolve()),
    sessionPlay: mockSessionPlay,
    addToQueue: vi.fn(() => Promise.resolve()),
    artUrl: vi.fn((id: string) => `/api/art/${id}`),
    getLibraryState: mockGetLibraryState,
    setLibraryState: mockSetLibraryState,
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mockGetLibraryState.mockResolvedValue({ path: [{ id: "0", title: "Library" }] });
  useDeviceStore.setState({ devices: [TEST_DEVICE], settingsDeviceId: "dev-1" });
  useUiStore.setState({ drawer: null });
});

describe("LibraryBrowser rendering", () => {
  it("renders search bar", () => {
    mockBrowse.mockResolvedValue({ items: [], total: 0 });
    renderWithProviders(<LibraryBrowser />);
    expect(screen.getByPlaceholderText("Search tracks, artists, albums...")).toBeInTheDocument();
  });

  it("shows root categories as a grid", async () => {
    mockBrowse.mockResolvedValue({
      items: [
        {
          type: "container",
          id: "1",
          parent_id: "0",
          title: "Artists",
          class: null,
          child_count: 50,
        },
        {
          type: "container",
          id: "2",
          parent_id: "0",
          title: "Albums",
          class: null,
          child_count: 30,
        },
        {
          type: "container",
          id: "3",
          parent_id: "0",
          title: "Genres",
          class: null,
          child_count: 10,
        },
        {
          type: "container",
          id: "4",
          parent_id: "0",
          title: "All Tracks",
          class: null,
          child_count: 200,
        },
      ],
      total: 4,
    });
    renderWithProviders(<LibraryBrowser />);
    await waitFor(() => {
      expect(screen.getByText("Artists")).toBeInTheDocument();
      expect(screen.getByText("Albums")).toBeInTheDocument();
      expect(screen.getByText("Genres")).toBeInTheDocument();
      expect(screen.getByText("All Tracks")).toBeInTheDocument();
    });
  });
});

describe("LibraryBrowser containers", () => {
  it("shows child count for containers", async () => {
    mockBrowse.mockResolvedValue({
      items: [
        {
          type: "container",
          id: "1",
          parent_id: "0",
          title: "Artists",
          class: null,
          child_count: 50,
        },
      ],
      total: 1,
    });
    renderWithProviders(<LibraryBrowser />);
    await waitFor(() => {
      expect(screen.getByText("50 items")).toBeInTheDocument();
    });
  });

  it('shows "Empty" when browse returns no items', async () => {
    mockBrowse.mockResolvedValue({ items: [], total: 0 });
    renderWithProviders(<LibraryBrowser />);
    await waitFor(() => {
      expect(screen.getByText("Empty")).toBeInTheDocument();
    });
  });
});

describe("LibraryBrowser navigation", () => {
  it("navigates into a container on click", async () => {
    mockBrowse
      .mockResolvedValueOnce({
        items: [
          {
            type: "container",
            id: "1",
            parent_id: "0",
            title: "Artists",
            class: null,
            child_count: 2,
          },
        ],
        total: 1,
      })
      .mockResolvedValueOnce({
        items: [
          {
            type: "container",
            id: "10",
            parent_id: "1",
            title: "Pink Floyd",
            class: null,
            child_count: 5,
          },
          {
            type: "container",
            id: "11",
            parent_id: "1",
            title: "Led Zeppelin",
            class: null,
            child_count: 8,
          },
        ],
        total: 2,
      });
    renderWithProviders(<LibraryBrowser />);
    await waitFor(() => screen.getByText("Artists"));
    fireEvent.click(screen.getByText("Artists"));
    await waitFor(() => {
      expect(screen.getByText("Pink Floyd")).toBeInTheDocument();
      expect(screen.getByText("Led Zeppelin")).toBeInTheDocument();
    });
  });

  it("shows breadcrumbs after navigating", async () => {
    mockBrowse
      .mockResolvedValueOnce({
        items: [{ type: "container", id: "1", parent_id: "0", title: "Artists", class: null }],
        total: 1,
      })
      .mockResolvedValueOnce({ items: [], total: 0 });
    renderWithProviders(<LibraryBrowser />);
    await waitFor(() => screen.getByText("Artists"));
    fireEvent.click(screen.getByText("Artists"));
    await waitFor(() => {
      expect(screen.getByText("Library")).toBeInTheDocument();
      // "Artists" appears both in breadcrumb — just check it exists
      expect(screen.getByText("Artists")).toBeInTheDocument();
    });
  });
});

describe("LibraryBrowser playback", () => {
  it("closes via callback after playing a track from the current library path", async () => {
    const onPlay = vi.fn();
    useUiStore.setState({ drawer: "library" });
    mockGetLibraryState.mockResolvedValue({
      path: [
        { id: "0", title: "Library" },
        { id: "1", title: "Artists" },
      ],
    });
    mockBrowse.mockResolvedValue({
      items: [
        {
          type: "track",
          id: "t1",
          parent_id: "1",
          title: "Wish You Were Here",
          artist: "Pink Floyd",
          album: "Wish You Were Here",
          class: null,
        },
      ],
      total: 1,
    });

    renderWithProviders(<LibraryBrowser onPlay={onPlay} />);
    await waitFor(() => screen.getByText("Wish You Were Here"));
    fireEvent.click(screen.getByText("Wish You Were Here"));

    await waitFor(() => {
      expect(mockSessionPlay).toHaveBeenCalledWith({
        source_id: "1",
        start_track_id: "t1",
      });
      expect(onPlay).toHaveBeenCalled();
    });
  });
});
