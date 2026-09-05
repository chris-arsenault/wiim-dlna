import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type QueueTrack } from "../../api/client";
import { usePlayerStore } from "../../stores/playerStore";

function useQueueActions() {
  const queryClient = useQueryClient();

  const handleRemove = useCallback(
    async (index: number) => {
      await api.removeFromQueue(index);
      queryClient.invalidateQueries({ queryKey: ["queue"] });
    },
    [queryClient]
  );

  const handleClear = useCallback(async () => {
    await api.clearQueue();
    queryClient.invalidateQueries({ queryKey: ["queue"] });
  }, [queryClient]);

  const handleDrop = useCallback(
    async (fromIndex: number, toIndex: number) => {
      if (fromIndex === toIndex) return;
      await api.moveInQueue(fromIndex, toIndex);
      queryClient.invalidateQueries({ queryKey: ["queue"] });
    },
    [queryClient]
  );

  return { handleRemove, handleClear, handleDrop };
}

export function QueueView() {
  const currentTrack = usePlayerStore((s) => s.currentTrack);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const touchDragRef = useRef<{ startIndex: number } | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["queue"],
    queryFn: () => api.getQueue(),
  });

  const { handleRemove, handleClear, handleDrop } = useQueueActions();
  const { handleTouchMove, handleTouchEnd, handleTouchStart } = useTouchDrag(
    touchDragRef,
    listRef,
    setDragIndex,
    setDragOverIndex,
    dragOverIndex,
    handleDrop
  );

  const tracks = data?.tracks ?? [];
  const position = data?.position ?? 0;

  return (
    <div className="space-y-3">
      <QueueHeader trackCount={tracks.length} onClear={handleClear} />
      <QueueContent
        isLoading={isLoading}
        tracks={tracks}
        position={position}
        currentTrack={currentTrack}
        dragIndex={dragIndex}
        dragOverIndex={dragOverIndex}
        listRef={listRef}
        handleTouchMove={handleTouchMove}
        handleTouchEnd={handleTouchEnd}
        setDragIndex={setDragIndex}
        setDragOverIndex={setDragOverIndex}
        handleDrop={handleDrop}
        handleTouchStart={handleTouchStart}
        handleRemove={handleRemove}
      />
      {tracks.length > 0 && (
        <div className="text-xs text-[var(--color-text-secondary)] text-center pt-2">
          {tracks.length} track{tracks.length !== 1 ? "s" : ""} in queue
        </div>
      )}
    </div>
  );
}

function useTouchDrag(
  touchDragRef: React.MutableRefObject<{ startIndex: number } | null>,
  listRef: React.RefObject<HTMLDivElement | null>,
  setDragIndex: (i: number | null) => void,
  setDragOverIndex: (i: number | null) => void,
  dragOverIndex: number | null,
  handleDrop: (from: number, to: number) => void
) {
  const getItemIndexAtPoint = useCallback(
    (y: number): number | null => {
      const list = listRef.current;
      if (!list) return null;
      const children = list.children;
      for (let i = 0; i < children.length; i++) {
        const rect = children[i].getBoundingClientRect();
        if (y >= rect.top && y <= rect.bottom) return i;
      }
      return null;
    },
    [listRef]
  );

  const handleTouchStart = useCallback(
    (index: number) => {
      touchDragRef.current = { startIndex: index };
      setDragIndex(index);
    },
    [touchDragRef, setDragIndex]
  );

  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      if (!touchDragRef.current) return;
      e.preventDefault();
      const touch = e.touches[0];
      const overIndex = getItemIndexAtPoint(touch.clientY);
      if (overIndex !== null) setDragOverIndex(overIndex);
    },
    [touchDragRef, getItemIndexAtPoint, setDragOverIndex]
  );

  const handleTouchEnd = useCallback(() => {
    if (touchDragRef.current && dragOverIndex !== null) {
      const from = touchDragRef.current.startIndex;
      if (from !== dragOverIndex) handleDrop(from, dragOverIndex);
    }
    touchDragRef.current = null;
    setDragIndex(null);
    setDragOverIndex(null);
  }, [touchDragRef, dragOverIndex, handleDrop, setDragIndex, setDragOverIndex]);

  return { handleTouchMove, handleTouchEnd, handleTouchStart };
}

function QueueHeader({ trackCount, onClear }: { trackCount: number; onClear: () => void }) {
  return (
    <div className="flex items-center justify-between">
      <h2 className="text-xl font-semibold">Queue</h2>
      {trackCount > 0 && (
        <button
          onClick={onClear}
          className="text-xs text-[var(--color-text-secondary)] hover:text-[var(--color-accent)] transition-colors px-2 py-1"
        >
          Clear all
        </button>
      )}
    </div>
  );
}

function QueueContent({
  isLoading,
  tracks,
  position,
  currentTrack,
  dragIndex,
  dragOverIndex,
  listRef,
  handleTouchMove,
  handleTouchEnd,
  setDragIndex,
  setDragOverIndex,
  handleDrop,
  handleTouchStart,
  handleRemove,
}: {
  isLoading: boolean;
  tracks: QueueTrack[];
  position: number;
  currentTrack: QueueTrack | null;
  dragIndex: number | null;
  dragOverIndex: number | null;
  listRef: React.RefObject<HTMLDivElement | null>;
  handleTouchMove: (e: React.TouchEvent) => void;
  handleTouchEnd: () => void;
  setDragIndex: (i: number | null) => void;
  setDragOverIndex: (i: number | null) => void;
  handleDrop: (from: number, to: number) => void;
  handleTouchStart: (index: number) => void;
  handleRemove: (index: number) => void;
}) {
  if (isLoading) {
    return (
      <div className="text-center py-12 text-[var(--color-text-secondary)] text-sm">Loading...</div>
    );
  }
  if (tracks.length === 0) return <QueueEmptyState />;
  return (
    <div
      ref={listRef}
      className="space-y-0.5"
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
    >
      {tracks.map((track, i) => (
        <QueueItem
          key={`${track.id}-${i}`}
          track={track}
          index={i}
          isPlaying={i === position && currentTrack?.id === track.id}
          isDragging={dragIndex === i}
          isDragOver={dragOverIndex === i}
          onRemove={() => handleRemove(i)}
          onDragStart={() => setDragIndex(i)}
          onDragOver={() => setDragOverIndex(i)}
          onDragEnd={() => {
            if (dragIndex !== null && dragOverIndex !== null && dragIndex !== dragOverIndex) {
              handleDrop(dragIndex, dragOverIndex);
            }
            setDragIndex(null);
            setDragOverIndex(null);
          }}
          onTouchDragStart={() => handleTouchStart(i)}
        />
      ))}
    </div>
  );
}

function QueueEmptyState() {
  return (
    <div className="text-center py-12">
      <div className="text-[var(--color-text-secondary)] text-sm">Queue is empty</div>
      <div className="text-[var(--color-text-secondary)] text-xs mt-1">
        Browse your library and tap a track to start playing
      </div>
    </div>
  );
}

function QueueItem({
  track,
  index,
  isPlaying,
  isDragging,
  isDragOver,
  onRemove,
  onDragStart,
  onDragOver,
  onDragEnd,
  onTouchDragStart,
}: {
  track: QueueTrack;
  index: number;
  isPlaying: boolean;
  isDragging: boolean;
  isDragOver: boolean;
  onRemove: () => void;
  onDragStart: () => void;
  onDragOver: () => void;
  onDragEnd: () => void;
  onTouchDragStart: () => void;
}) {
  return (
    <div
      draggable
      onDragStart={onDragStart}
      onDragOver={(e) => {
        e.preventDefault();
        onDragOver();
      }}
      onDragEnd={onDragEnd}
      className={`flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors ${
        isPlaying ? "bg-[var(--color-accent)]/10" : "hover:bg-[var(--color-surface-hover)]"
      } ${isDragging ? "opacity-40" : ""} ${isDragOver ? "border-t-2 border-[var(--color-accent)]" : ""}`}
    >
      <div
        className="cursor-grab text-white/20 hover:text-white/50 shrink-0 touch-none"
        onTouchStart={(e) => {
          e.stopPropagation();
          onTouchDragStart();
        }}
      >
        <GripIcon />
      </div>
      <div
        className={`w-6 text-center text-xs shrink-0 ${isPlaying ? "text-[var(--color-accent)]" : "text-[var(--color-text-secondary)]"}`}
      >
        {isPlaying ? <PlayingIndicator /> : index + 1}
      </div>
      <div className="flex-1 min-w-0">
        <div
          className={`text-sm truncate ${isPlaying ? "text-[var(--color-accent)] font-medium" : ""}`}
        >
          {track.title}
        </div>
        <div className="text-xs text-[var(--color-text-secondary)] truncate">
          {[track.artist, track.album].filter(Boolean).join(" — ")}
        </div>
      </div>
      {track.duration && (
        <div className="text-xs text-[var(--color-text-secondary)] shrink-0">{track.duration}</div>
      )}
      <button
        onClick={(e) => {
          e.stopPropagation();
          onRemove();
        }}
        className="p-1.5 text-[var(--color-text-secondary)] hover:text-red-400 transition-colors shrink-0"
      >
        <XIcon />
      </button>
    </div>
  );
}

function PlayingIndicator() {
  return (
    <div className="flex items-end justify-center gap-0.5 h-4">
      {[
        { height: "60%", delay: "0ms" },
        { height: "100%", delay: "150ms" },
        { height: "40%", delay: "300ms" },
      ].map(({ height, delay }, i) => (
        <div
          key={i}
          className="w-0.5 bg-[var(--color-accent)] animate-pulse playing-bar"
          ref={(el) => {
            if (el) {
              el.style.setProperty("--bar-height", height);
              el.style.setProperty("--bar-delay", delay);
            }
          }}
        />
      ))}
    </div>
  );
}

function XIcon() {
  return (
    <svg
      width="14"
      height="14"
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

function GripIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
      <circle cx="9" cy="5" r="1.5" />
      <circle cx="15" cy="5" r="1.5" />
      <circle cx="9" cy="12" r="1.5" />
      <circle cx="15" cy="12" r="1.5" />
      <circle cx="9" cy="19" r="1.5" />
      <circle cx="15" cy="19" r="1.5" />
    </svg>
  );
}
