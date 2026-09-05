import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type LibraryItem, type ContainerInfo } from "../../api/client";
import { selectPlaybackDevice, useDeviceStore } from "../../stores/deviceStore";
import { usePlayerStore } from "../../stores/playerStore";
import { LIBRARY_ROOT, type BreadcrumbEntry } from "../../stores/uiStore";
import { BulkAlbumArtistDialog, RenameArtistDialog } from "./TrackEditor";
import { ItemList } from "./LibraryItemList";
import { AddToPlaylistDialog } from "../playlists/AddToPlaylistDialog";
import {
  SearchIcon,
  XIcon,
  ChevronLeftIcon,
  PlayIcon,
  EditIcon,
  ListPlusIcon,
} from "./LibraryIcons";

const CATEGORY_ICONS: Record<string, string> = {
  Artists: "\uD83C\uDFA4",
  "Album Artists": "\uD83C\uDFB6",
  Albums: "\uD83D\uDCBF",
  Genres: "\uD83C\uDFB5",
  "All Tracks": "\u266B",
};

function SearchBar({ query, onChange }: { query: string; onChange: (value: string) => void }) {
  return (
    <div className="relative">
      <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-text-secondary)]" />
      <input
        type="text"
        placeholder="Search tracks, artists, albums..."
        value={query}
        onChange={(e) => onChange(e.target.value)}
        className="w-full bg-[var(--color-surface-elevated)] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-sm outline-none focus:border-[var(--color-accent)] transition-colors placeholder:text-[var(--color-text-secondary)]"
      />
      {query && (
        <button
          onClick={() => onChange("")}
          className="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
        >
          <XIcon />
        </button>
      )}
    </div>
  );
}

function Breadcrumbs({
  path,
  onBack,
  onNavigate,
}: {
  path: BreadcrumbEntry[];
  onBack: () => void;
  onNavigate: (index: number) => void;
}) {
  return (
    <div className="flex items-center gap-1 text-sm overflow-x-auto">
      <button onClick={onBack} className="text-[var(--color-text-secondary)] shrink-0 p-1">
        <ChevronLeftIcon />
      </button>
      {path.map((entry, i) => (
        <span key={entry.id} className="flex items-center gap-1 shrink-0">
          {i > 0 && <span className="text-[var(--color-text-secondary)]">/</span>}
          <button
            onClick={() => onNavigate(i)}
            className={
              i === path.length - 1
                ? "text-[var(--color-text-primary)] font-medium"
                : "text-[var(--color-text-secondary)]"
            }
          >
            {entry.title}
          </button>
        </span>
      ))}
    </div>
  );
}

function useLibraryData(currentId: string, searching: boolean, searchQuery: string) {
  const browseQuery = useQuery({
    queryKey: ["library", "browse", currentId],
    queryFn: () => api.browse(currentId),
    enabled: !searching,
  });

  const searchQueryResult = useQuery({
    queryKey: ["library", "search", searchQuery],
    queryFn: () => api.search(searchQuery),
    enabled: searching && searchQuery.length >= 2,
  });

  const containerInfo = browseQuery.data?.container;
  const items = searching ? searchQueryResult.data?.items : browseQuery.data?.items;
  const isLoading = searching ? searchQueryResult.isLoading : browseQuery.isLoading;

  return { containerInfo, items, isLoading };
}

export function LibraryBrowser({ onPlay }: { onPlay?: () => void }) {
  const queryClient = useQueryClient();
  const [searchQuery, setSearchQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const libraryState = useQuery({
    queryKey: ["library-state"],
    queryFn: () => api.getLibraryState(),
  });
  const saveLibraryState = useMutation({
    mutationFn: (nextPath: BreadcrumbEntry[]) => api.setLibraryState(nextPath),
  });

  const path = libraryState.data?.path ?? [LIBRARY_ROOT];
  const currentPath = path.length ? path : [LIBRARY_ROOT];
  const currentId = currentPath[currentPath.length - 1].id;
  const { containerInfo, items, isLoading } = useLibraryData(currentId, searching, searchQuery);

  const setPath = (nextPath: BreadcrumbEntry[]) => {
    const normalized = nextPath.length ? nextPath : [LIBRARY_ROOT];
    queryClient.setQueryData(["library-state"], { path: normalized });
    saveLibraryState.mutate(normalized);
  };

  const navigateTo = (item: LibraryItem) => {
    if (item.type !== "container") return;
    setPath([...currentPath, { id: item.id, title: item.title ?? "Unknown" }]);
    setSearching(false);
    setSearchQuery("");
  };

  const handleSearchChange = (value: string) => {
    setSearchQuery(value);
    setSearching(value.length > 0);
  };

  const showBreadcrumbs = !searching && currentPath.length > 1;
  const showHeader = !searching && containerInfo && (containerInfo.artist || containerInfo.album);

  return (
    <div className="space-y-3">
      <SearchBar query={searchQuery} onChange={handleSearchChange} />
      {showBreadcrumbs && (
        <Breadcrumbs
          path={currentPath}
          onBack={() => setPath(currentPath.slice(0, -1))}
          onNavigate={(i) => {
            setPath(currentPath.slice(0, i + 1));
            setSearching(false);
            setSearchQuery("");
          }}
        />
      )}
      {showHeader && <ContainerHeader info={containerInfo} onPlay={onPlay} />}
      <LibraryContent
        isLoading={isLoading}
        items={items}
        searching={searching}
        isRoot={currentId === "0"}
        containerId={currentId}
        onSelect={navigateTo}
        onPlay={onPlay}
      />
    </div>
  );
}

function ContainerHeaderInfo({
  info,
  isAlbum,
  isArtist,
}: {
  info: ContainerInfo;
  isAlbum: boolean;
  isArtist: boolean;
}) {
  if (isArtist) return <div className="text-base font-semibold truncate">{info.artist}</div>;
  if (!isAlbum) return null;
  return (
    <>
      <div className="text-base font-semibold truncate">{info.album}</div>
      {info.artist && (
        <div className="text-sm text-[var(--color-text-secondary)] truncate">{info.artist}</div>
      )}
    </>
  );
}

function ContainerHeaderActions({
  isAlbum,
  isArtist,
  canPlay,
  onEditAlbumArtist,
  onRename,
  onPlayAll,
  onAddToPlaylist,
}: {
  isAlbum: boolean;
  isArtist: boolean;
  canPlay: boolean;
  onEditAlbumArtist: () => void;
  onRename: () => void;
  onPlayAll: () => void;
  onAddToPlaylist: () => void;
}) {
  return (
    <div className="flex items-center gap-1.5 shrink-0">
      <button
        onClick={onAddToPlaylist}
        className="flex items-center gap-1 px-2.5 py-1.5 rounded-full border border-white/10 text-xs font-medium text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] transition-colors"
        title="Add all tracks to a playlist"
      >
        <ListPlusIcon />
        Playlist
      </button>
      {(isAlbum || isArtist) && (
        <button
          onClick={onEditAlbumArtist}
          className="flex items-center gap-1 px-2.5 py-1.5 rounded-full border border-white/10 text-xs font-medium text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] transition-colors"
          title="Set album artist for all tracks"
        >
          <EditIcon />
          Album Artist
        </button>
      )}
      {isArtist && (
        <button
          onClick={onRename}
          className="flex items-center gap-1 px-2.5 py-1.5 rounded-full border border-white/10 text-xs font-medium text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] transition-colors"
          title="Rename this artist"
        >
          <EditIcon />
          Rename
        </button>
      )}
      {canPlay && (
        <button
          onClick={onPlayAll}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-[var(--color-accent)] text-white text-xs font-medium active:scale-95 transition-transform"
          title="Play all"
        >
          <PlayIcon />
          Play All
        </button>
      )}
    </div>
  );
}

function ContainerHeader({ info, onPlay }: { info: ContainerInfo; onPlay?: () => void }) {
  const hasOutput = useDeviceStore((state) => selectPlaybackDevice(state.devices) != null);
  const setPlaying = usePlayerStore((s) => s.setPlaying);
  const [showAlbumArtist, setShowAlbumArtist] = useState(false);
  const [showRename, setShowRename] = useState(false);
  const [showAddToPlaylist, setShowAddToPlaylist] = useState(false);

  const isAlbum = info.class === "object.container.album.musicAlbum";
  const isArtist = info.class === "object.container.person.musicArtist";

  const handleQueueAll = async () => {
    if (!hasOutput) return;
    await api.sessionPlay({ source_id: info.id });
    setPlaying(true);
    onPlay?.();
  };

  return (
    <>
      <div className="bg-[var(--color-surface-elevated)] rounded-xl px-4 py-3 flex items-center gap-2">
        <div className="flex-1 min-w-0">
          <ContainerHeaderInfo info={info} isAlbum={isAlbum} isArtist={isArtist} />
        </div>
        <ContainerHeaderActions
          isAlbum={isAlbum}
          isArtist={isArtist}
          canPlay={hasOutput && (isArtist || isAlbum)}
          onEditAlbumArtist={() => setShowAlbumArtist(true)}
          onRename={() => setShowRename(true)}
          onPlayAll={handleQueueAll}
          onAddToPlaylist={() => setShowAddToPlaylist(true)}
        />
      </div>
      {showAddToPlaylist && (
        <AddToPlaylistDialog
          trackIds={[info.id]}
          label={info.album ?? info.artist ?? "Selection"}
          onClose={() => setShowAddToPlaylist(false)}
        />
      )}
      {showAlbumArtist && (
        <BulkAlbumArtistDialog
          containerId={info.id}
          currentArtist={info.artist}
          onClose={() => setShowAlbumArtist(false)}
        />
      )}
      {showRename && (
        <RenameArtistDialog initialFrom={info.artist} onClose={() => setShowRename(false)} />
      )}
    </>
  );
}

function LibraryContent({
  isLoading,
  items,
  searching,
  isRoot,
  containerId,
  onSelect,
  onPlay,
}: {
  isLoading: boolean;
  items: LibraryItem[] | undefined;
  searching: boolean;
  isRoot: boolean;
  containerId: string;
  onSelect: (item: LibraryItem) => void;
  onPlay?: () => void;
}) {
  if (isLoading) {
    return (
      <div className="text-center py-12 text-[var(--color-text-secondary)] text-sm">Loading...</div>
    );
  }
  if (!items?.length) {
    return (
      <div className="text-center py-12 text-[var(--color-text-secondary)] text-sm">
        {searching ? "No results found" : "Empty"}
      </div>
    );
  }
  if (isRoot && !searching) {
    return <CategoryGrid items={items} onSelect={onSelect} />;
  }
  return <ItemList items={items} onSelect={onSelect} containerId={containerId} onPlay={onPlay} />;
}

function CategoryGrid({
  items,
  onSelect,
}: {
  items: LibraryItem[];
  onSelect: (item: LibraryItem) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-3">
      {items.map((item) => (
        <button
          key={item.id}
          onClick={() => onSelect(item)}
          className="bg-[var(--color-surface-elevated)] rounded-xl p-5 text-left hover:bg-[var(--color-surface-hover)] transition-colors active:scale-[0.98]"
        >
          <div className="text-2xl mb-2">{CATEGORY_ICONS[item.title ?? ""] ?? "\uD83D\uDCC1"}</div>
          <div className="text-sm font-medium">{item.title}</div>
          {item.child_count != null && (
            <div className="text-xs text-[var(--color-text-secondary)] mt-0.5">
              {item.child_count} {item.child_count === 1 ? "item" : "items"}
            </div>
          )}
        </button>
      ))}
    </div>
  );
}
