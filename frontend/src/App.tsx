import { useEffect, useState, type ReactNode } from "react";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "framer-motion";
import { api, type SessionInfo } from "./api/client";
import { setApiAuthToken } from "./api/client";
import { Login } from "./components/Login";
import { useAuth } from "./hooks/useAuth";
import { BottomNav } from "./components/layout/BottomNav";
import { Sidebar } from "./components/layout/Sidebar";
import { NowPlaying } from "./components/player/NowPlaying";
import { LibraryBrowser } from "./components/library/LibraryBrowser";
import { PlaylistsView } from "./components/playlists/PlaylistsView";
import { QueueView } from "./components/queue/QueueView";
import { DeviceManager } from "./components/devices/DeviceManager";
import { EQSettings } from "./components/devices/EQSettings";
import { useDeviceStore } from "./stores/deviceStore";
import { usePlayerStore } from "./stores/playerStore";
import { useUiStore } from "./stores/uiStore";
import { useSSE } from "./hooks/useSSE";
import { useMediaSession } from "./hooks/useMediaSession";
import { config } from "./config";

const queryClient = new QueryClient();

const DRAWER_TITLES: Record<string, string> = {
  library: "Library",
  playlists: "Playlists",
  queue: "Queue",
  devices: "Speakers",
  settings: "EQ",
};

function DrawerContent({ drawer, onClose }: { drawer: string; onClose: () => void }) {
  return (
    <>
      {drawer === "library" && <LibraryBrowser onPlay={onClose} />}
      {drawer === "playlists" && <PlaylistsView onPlay={onClose} />}
      {drawer === "queue" && <QueueView />}
      {drawer === "devices" && <DeviceManager />}
      {drawer === "settings" && <EQSettings />}
    </>
  );
}

function AppContent() {
  const drawer = useUiStore((s) => s.drawer);
  const setDrawer = useUiStore((s) => s.setDrawer);
  const toggleDrawer = useUiStore((s) => s.toggleDrawer);

  useMediaSession();
  useInitialDeviceFetch();
  useSSEHandlers();

  return (
    <div className="app-layout">
      <Sidebar active={drawer} onNavigate={toggleDrawer} />
      <div className="app-content">
        <DesktopDrawer drawer={drawer} onClose={() => setDrawer(null)} />
        <div className="app-player">
          <NowPlaying />
        </div>
        <MobileDrawer drawer={drawer} onClose={() => setDrawer(null)} />
      </div>
      <BottomNav active={drawer} onNavigate={toggleDrawer} />
    </div>
  );
}

function useInitialDeviceFetch() {
  const setDevices = useDeviceStore((s) => s.setDevices);
  useEffect(() => {
    api.getDevices().then(setDevices).catch(console.error);
  }, [setDevices]);
}

function useSSEHandlers() {
  const setDevices = useDeviceStore((s) => s.setDevices);
  const updateDevice = useDeviceStore((s) => s.updateDevice);
  const setPlaying = usePlayerStore((s) => s.setPlaying);
  const setCurrentTrack = usePlayerStore((s) => s.setCurrentTrack);
  const setSession = usePlayerStore((s) => s.setSession);
  const qc = useQueryClient();

  useSSE({
    playback_state: (data) => handlePlaybackState(data),
    devices_changed: (data) => {
      const devices = (data as { devices: Parameters<typeof setDevices>[0] }).devices;
      setDevices(devices);
    },
    device_state: (data) => {
      const { device_id, ...update } = data as { device_id: string; [key: string]: unknown };
      updateDevice(device_id, update);
    },
    volume_changed: (data) => {
      const { device_id, volume } = data as { device_id: string; volume: number };
      updateDevice(device_id, { volume });
    },
    mute_changed: (data) => {
      const { device_id, muted } = data as { device_id: string; muted: boolean };
      updateDevice(device_id, { muted });
    },
    track_changed: (data) => handleTrackChanged(data, setCurrentTrack),
    playback_started: () => setPlaying(true),
    playback_stopped: () => setPlaying(false),
    queue_ended: () => {
      setPlaying(false);
      setCurrentTrack(null);
    },
    session_started: (data) => {
      const { session } = data as { target_id: string; session: SessionInfo };
      setSession(session);
      setPlaying(true);
    },
    session_ended: () => {
      setSession(null);
      setPlaying(false);
      setCurrentTrack(null);
    },
    sleep_timer_expired: () => {
      setPlaying(false);
      usePlayerStore.setState({ sleepRemaining: null });
    },
    sleep_timer_changed: (data) => handleSleepTimerChanged(data),
    queue_changed: (data) => handleQueueChanged(data, qc),
  });
}

function handlePlaybackState(data: unknown) {
  const state = data as {
    target_id: string;
    playing: boolean;
    current_track: {
      id: string;
      title: string;
      artist: string | null;
      album: string | null;
      duration: string | null;
      stream_url: string | null;
    } | null;
    elapsed_seconds: number;
    duration_seconds: number;
    shuffle_mode: string;
    repeat_mode: string;
    session?: SessionInfo | null;
    allowed_actions?: string[] | null;
  };
  if (state.target_id !== "playing") return;
  const session = state.session ?? null;
  const currentId = usePlayerStore.getState().currentTrack?.id;
  const trackChanged = state.current_track && state.current_track.id !== currentId;
  usePlayerStore.setState({
    playing: state.playing,
    elapsedSeconds: state.elapsed_seconds,
    durationSeconds: state.duration_seconds,
    session,
    shuffleMode: session ? session.shuffle_mode : state.shuffle_mode,
    repeatMode: session ? session.repeat_mode : state.repeat_mode,
    allowedActions: state.allowed_actions ?? [],
    ...(state.current_track ? { currentTrack: state.current_track } : {}),
    ...(trackChanged ? { rating: 0 } : {}),
  });
}

function handleTrackChanged(
  data: unknown,
  setCurrentTrack: (
    track: {
      id: string;
      title: string;
      artist: string | null;
      album: string | null;
      duration: string | null;
      stream_url: string | null;
    } | null
  ) => void
) {
  const { target_id, track } = data as {
    target_id: string;
    track: { id: string; title: string; artist: string | null };
  };
  if (target_id === "playing" && track) {
    setCurrentTrack({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: null,
      duration: null,
      stream_url: null,
    });
  }
}

function handleSleepTimerChanged(data: unknown) {
  const { remaining_seconds, target_id } = data as {
    target_id: string;
    remaining_seconds: number | null;
  };
  if (target_id !== "playing") return;
  usePlayerStore.setState({ sleepRemaining: remaining_seconds });
}

function handleQueueChanged(data: unknown, qc: ReturnType<typeof useQueryClient>) {
  const { target_id, tracks, position } = data as {
    target_id: string;
    tracks?: unknown[];
    position?: number;
  };
  if (target_id === "playing" && tracks !== undefined) {
    qc.setQueryData(["queue"], { tracks, position });
  } else {
    qc.invalidateQueries({ queryKey: ["queue"] });
  }
}

function DesktopDrawer({ drawer, onClose }: { drawer: string | null; onClose: () => void }) {
  if (!drawer) return null;
  return (
    <div className="app-drawer-panel hidden md:flex flex-col bg-[var(--color-surface)]">
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/5 shrink-0">
        <h2 className="text-lg font-semibold">{DRAWER_TITLES[drawer]}</h2>
        <button
          onClick={onClose}
          className="p-1.5 text-white/40 hover:text-white rounded-lg hover:bg-white/5"
        >
          <XIcon />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-4 py-3">
        <DrawerContent drawer={drawer} onClose={onClose} />
      </div>
    </div>
  );
}

function MobileDrawer({ drawer, onClose }: { drawer: string | null; onClose: () => void }) {
  return (
    <AnimatePresence>
      {drawer && (
        <motion.div
          key="drawer"
          className="absolute inset-0 z-30 flex flex-col md:hidden"
          initial={{ y: "100%" }}
          animate={{ y: 0 }}
          exit={{ y: "100%" }}
          transition={{ type: "spring", damping: 30, stiffness: 300 }}
        >
          <div className="bg-[var(--color-surface)] rounded-t-2xl flex flex-col flex-1 min-h-0">
            <button
              type="button"
              className="flex items-center justify-center pt-3 pb-1 shrink-0 cursor-pointer"
              onClick={onClose}
            >
              <div className="w-10 h-1 rounded-full bg-white/20" />
            </button>
            <div className="flex items-center justify-between px-4 py-2 shrink-0">
              <h2 className="text-lg font-semibold">{DRAWER_TITLES[drawer]}</h2>
              <button onClick={onClose} className="p-1.5 text-white/40 hover:text-white">
                <XIcon />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto px-4 pb-4">
              <DrawerContent drawer={drawer} onClose={onClose} />
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function XIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
    >
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppGate />
    </QueryClientProvider>
  );
}

function AppGate() {
  const { auth, authActions } = useAuth();
  const token = config.authRequired && auth.status === "signedIn" ? auth.token : "";
  let content: ReactNode;

  if (!config.authRequired || auth.status === "signedIn") {
    content = <AppContent />;
  } else if (auth.status === "loading") {
    content = <div className="login-screen">Loading…</div>;
  } else {
    content = <Login auth={auth} authActions={authActions} />;
  }

  return <ApiAuthBoundary token={token}>{content}</ApiAuthBoundary>;
}

function ApiAuthBoundary({ token, children }: { token: string; children: ReactNode }) {
  const [readyToken, setReadyToken] = useState<string | null>(null);

  useEffect(() => {
    setApiAuthToken(token);
    setReadyToken(token);
  }, [token]);

  if (readyToken !== token) {
    return <div className="login-screen">Loading…</div>;
  }
  return children;
}
