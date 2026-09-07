import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, fireEvent, waitFor } from "@testing-library/react";
import { renderWithProviders } from "./test-utils";
import App from "./App";
import { api, setApiAuthToken } from "./api/client";
import { useDeviceStore } from "./stores/deviceStore";
import { usePlayerStore } from "./stores/playerStore";
import { useUiStore } from "./stores/uiStore";

vi.mock("./config", () => ({
  config: { authRequired: true },
}));

// Mock api
vi.mock("./api/client", () => ({
  api: {
    getDevices: vi.fn(() => Promise.resolve([])),
    getOutputState: vi.fn(() =>
      Promise.resolve({ required: false, in_progress: false, error: null })
    ),
    browse: vi.fn(() => Promise.resolve({ items: [], total: 0 })),
    getQueue: vi.fn(() => Promise.resolve({ tracks: [], position: 0 })),
    pause: vi.fn(() => Promise.resolve()),
    resume: vi.fn(() => Promise.resolve()),
    getSleepTimer: vi.fn(() => Promise.resolve({ remaining_seconds: null })),
    getLibraryState: vi.fn(() => Promise.resolve({ path: [{ id: "0", title: "Library" }] })),
    artUrl: vi.fn((id: string) => `/api/art/${id}`),
  },
  setApiAuthToken: vi.fn(),
}));

// Signed-in auth so App renders the player, not the login gate.
vi.mock("./hooks/useAuth", () => ({
  useAuth: () => ({
    auth: { status: "signedIn", token: "test-token", username: "tester" },
    authActions: { signIn: vi.fn(), confirmMfa: vi.fn(), signOut: vi.fn() },
  }),
}));

// Mock SSE
vi.mock("./hooks/useSSE", () => ({
  useSSE: vi.fn(),
}));

// Mock art color hook
vi.mock("./hooks/useArtColor", () => ({
  useArtColor: () => ({ dominant: "#6366f1", muted: "#2d2b55" }),
}));

// Mock framer-motion to avoid animation issues in tests
vi.mock("framer-motion", () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => children,
  motion: {
    div: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
  },
}));

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useDeviceStore.setState({
      devices: [],
      outputRecovery: { required: false, in_progress: false, error: null },
      settingsDeviceId: null,
    });
    usePlayerStore.setState({ playing: false, currentTrack: null, session: null });
    useUiStore.setState({ drawer: null });
  });

  it("installs the signed-in token before the initial device request", async () => {
    renderWithProviders(<App />);

    await waitFor(() => expect(api.getDevices).toHaveBeenCalled());
    expect(setApiAuthToken).toHaveBeenCalledWith("test-token");
    expect(vi.mocked(setApiAuthToken).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(api.getDevices).mock.invocationCallOrder[0]
    );
  });

  it("shows player as main view", () => {
    renderWithProviders(<App />);
    expect(screen.getByText("Nothing playing")).toBeInTheDocument();
  });

  it("renders bottom navigation", () => {
    renderWithProviders(<App />);
    const nav = screen.getByRole("navigation");
    expect(nav).toHaveTextContent("Library");
    expect(nav).toHaveTextContent("Queue");
    expect(nav).toHaveTextContent("Speakers");
    expect(nav).toHaveTextContent("EQ");
  });

  it("opens Library drawer on nav click", async () => {
    renderWithProviders(<App />);
    fireEvent.click(screen.getAllByText("Library")[0]);
    await waitFor(() => {
      // Both desktop and mobile drawers render in JSDOM
      expect(
        screen.getAllByPlaceholderText("Search tracks, artists, albums...").length
      ).toBeGreaterThanOrEqual(1);
    });
  });

  it("opens Speakers drawer on nav click", () => {
    renderWithProviders(<App />);
    fireEvent.click(screen.getAllByText("Speakers")[0]);
    expect(screen.getAllByText("Discovering WiiM speakers...").length).toBeGreaterThanOrEqual(1);
  });

  it("toggles drawer closed on second click", () => {
    renderWithProviders(<App />);
    fireEvent.click(screen.getAllByText("Speakers")[0]);
    expect(screen.getAllByText("Discovering WiiM speakers...").length).toBeGreaterThanOrEqual(1);
    fireEvent.click(screen.getAllByText("Speakers")[0]);
    expect(screen.getByText("Nothing playing")).toBeInTheDocument();
  });
});
