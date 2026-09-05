import { useState } from "react";
import { api, type LibraryItem } from "../../api/client";
import { selectPlaybackDevice, useDeviceStore } from "../../stores/deviceStore";
import { usePlayerStore } from "../../stores/playerStore";
import { TrackEditor } from "./TrackEditor";
import { AddToPlaylistDialog } from "../playlists/AddToPlaylistDialog";
import {
  ChevronRightIcon,
  PlayIcon,
  PlusIcon,
  FolderIcon,
  EditIcon,
  ListPlusIcon,
} from "./LibraryIcons";

const ROW_ICON_BUTTON =
  "shrink-0 p-1.5 rounded-full text-[var(--color-text-secondary)] hover:text-[var(--color-accent)] hover:bg-[var(--color-accent)]/10 transition-colors opacity-0 group-hover:opacity-100";

function containerSubtitle(item: LibraryItem): string {
  if (item.artist && item.album) {
    return `${item.artist} \u2014 ${item.child_count ?? 0} tracks`;
  }
  return `${item.child_count ?? 0} items`;
}

function ArtThumb({ trackId, fallback }: { trackId: string; fallback: React.ReactNode }) {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return (
      <div className="w-10 h-10 rounded-lg bg-[var(--color-accent)]/10 flex items-center justify-center shrink-0 group-hover:bg-[var(--color-accent)]/20 transition-colors">
        {fallback}
      </div>
    );
  }
  return (
    <img
      src={api.artUrl(trackId)}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className="w-10 h-10 rounded-lg object-cover shrink-0"
    />
  );
}

function ContainerRow({
  item,
  onSelect,
  onPlayAll,
  onAddToPlaylist,
  showPlayAll,
}: {
  item: LibraryItem;
  onSelect: () => void;
  onPlayAll: () => void;
  onAddToPlaylist: () => void;
  showPlayAll: boolean;
}) {
  return (
    <>
      <button onClick={onSelect} className="flex items-center gap-3 flex-1 min-w-0 text-left">
        <div className="w-10 h-10 rounded-lg bg-[var(--color-surface-elevated)] flex items-center justify-center text-[var(--color-text-secondary)] shrink-0">
          <FolderIcon />
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">{item.title ?? "Unknown"}</div>
          <div className="text-xs text-[var(--color-text-secondary)] truncate">
            {containerSubtitle(item)}
          </div>
        </div>
        <ChevronRightIcon className="text-[var(--color-text-secondary)] shrink-0" />
      </button>
      <button onClick={onAddToPlaylist} className={ROW_ICON_BUTTON} title="Add to playlist">
        <ListPlusIcon />
      </button>
      {showPlayAll && (
        <button onClick={onPlayAll} className={ROW_ICON_BUTTON} title="Play all">
          <PlayIcon />
        </button>
      )}
    </>
  );
}

function TrackRow({
  item,
  onPlay,
  onEdit,
  onAddToQueue,
  onAddToPlaylist,
}: {
  item: LibraryItem;
  onPlay: () => void;
  onEdit: () => void;
  onAddToQueue: () => void;
  onAddToPlaylist: () => void;
}) {
  return (
    <>
      <button
        onClick={onPlay}
        className="flex items-center gap-3 flex-1 min-w-0 text-left"
        title="Play"
      >
        <ArtThumb trackId={item.id} fallback={<PlayIcon />} />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">
            {item.track_number && (
              <span className="text-[var(--color-text-secondary)] mr-1.5">
                {item.track_number}.
              </span>
            )}
            {item.title ?? "Unknown"}
          </div>
          <div className="text-xs text-[var(--color-text-secondary)] truncate">
            {[item.artist, item.album].filter(Boolean).join(" \u2014 ") || "\u00A0"}
          </div>
        </div>
      </button>
      {item.duration && (
        <span className="text-xs text-[var(--color-text-secondary)] shrink-0">{item.duration}</span>
      )}
      <button onClick={onEdit} className={ROW_ICON_BUTTON} title="Edit metadata">
        <EditIcon />
      </button>
      <button onClick={onAddToPlaylist} className={ROW_ICON_BUTTON} title="Add to playlist">
        <ListPlusIcon />
      </button>
      <button onClick={onAddToQueue} className={ROW_ICON_BUTTON} title="Add to queue">
        <PlusIcon />
      </button>
    </>
  );
}

function useItemActions(containerId: string, onPlay?: () => void) {
  const hasOutput = useDeviceStore((state) => selectPlaybackDevice(state.devices) != null);
  const setCurrentTrack = usePlayerStore((s) => s.setCurrentTrack);
  const setPlaying = usePlayerStore((s) => s.setPlaying);

  const handleTrackPlay = async (item: LibraryItem) => {
    if (!hasOutput) return;
    await api.sessionPlay({ source_id: containerId, start_track_id: item.id });
    setCurrentTrack({
      id: item.id,
      title: item.title ?? "Unknown",
      artist: item.artist ?? null,
      album: item.album ?? null,
      duration: item.duration ?? null,
      stream_url: item.stream_url ?? null,
    });
    setPlaying(true);
    onPlay?.();
  };

  const handleAddToQueue = async (item: LibraryItem) => {
    await api.addToQueue([item.id]);
  };

  const handlePlayContainer = async (item: LibraryItem) => {
    if (!hasOutput) return;
    await api.sessionPlay({ source_id: item.id });
    setPlaying(true);
    onPlay?.();
  };

  return { handleTrackPlay, handleAddToQueue, handlePlayContainer };
}

export function ItemList({
  items,
  onSelect,
  containerId,
  onPlay,
}: {
  items: LibraryItem[];
  onSelect: (item: LibraryItem) => void;
  containerId: string;
  onPlay?: () => void;
}) {
  const hasOutput = useDeviceStore((state) => selectPlaybackDevice(state.devices) != null);
  const [editingTrack, setEditingTrack] = useState<LibraryItem | null>(null);
  const [playlistTarget, setPlaylistTarget] = useState<LibraryItem | null>(null);
  const { handleTrackPlay, handleAddToQueue, handlePlayContainer } = useItemActions(
    containerId,
    onPlay
  );

  return (
    <>
      <div className="space-y-0.5">
        {items.map((item) => (
          <div
            key={item.id}
            className="flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-[var(--color-surface-hover)] transition-colors group"
          >
            {item.type === "container" ? (
              <ContainerRow
                item={item}
                onSelect={() => onSelect(item)}
                onPlayAll={() => handlePlayContainer(item)}
                onAddToPlaylist={() => setPlaylistTarget(item)}
                showPlayAll={hasOutput}
              />
            ) : (
              <TrackRow
                item={item}
                onPlay={() => handleTrackPlay(item)}
                onEdit={() => setEditingTrack(item)}
                onAddToQueue={() => handleAddToQueue(item)}
                onAddToPlaylist={() => setPlaylistTarget(item)}
              />
            )}
          </div>
        ))}
      </div>
      {editingTrack && <TrackEditor track={editingTrack} onClose={() => setEditingTrack(null)} />}
      {playlistTarget && (
        <AddToPlaylistDialog
          trackIds={[playlistTarget.id]}
          label={playlistTarget.title ?? "Selection"}
          onClose={() => setPlaylistTarget(null)}
        />
      )}
    </>
  );
}
