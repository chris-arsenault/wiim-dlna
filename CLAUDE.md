# CLAUDE.md

## Project Overview

Airwave — a monorepo for a WiiM audio ecosystem: a unified Rust server (DLNA media server + control plane) and a web-based frontend for WiiM devices. Deployed to TrueNAS via Docker Compose + Komodo.

### Components

| Directory | Language | Purpose |
|-----------|----------|---------|
| `backend/` | Rust | Unified DLNA/UPnP MediaServer + control plane — SSDP, SOAP, HTTP streaming, device management, playback, queue/session engine, playlists, EQ, metadata editing, SSE |
| `frontend/` | React/TypeScript (Vite, pnpm) | Mobile-first web UI — library browser, now playing, queue, playlists, WiiM output selection, metadata editing |

## Build & Run

### Backend (Rust)

```bash
cd backend
cargo build --release
cargo test
cargo fmt --all --check
cargo clippy -- -D warnings
cargo run -- config.toml
```

### Frontend (React)

```bash
cd frontend
pnpm install
pnpm run dev                   # vite dev server
pnpm run lint && pnpm run typecheck
pnpm run test -- --run
```

### Full Stack (Docker)

```bash
docker compose up -d
```

### Lint (all)

```bash
make ci
```

## Platform Integration

- **CI**: Shared reusable workflow (`chris-arsenault/ahara/.github/workflows/ci.yml@main`)
- **Deploy**: TrueNAS via Komodo (`truenas: true` in `platform.yml`)
- **Project registration**: `ahara-control/infrastructure/terraform/project-airwave.tf`
- **No external database** (uses local SQLite). Runtime still uses host networking
  for SSDP; public access is via the shared `services.ahara.io` reverse proxy
  with Cognito auth at the edge.

## Architecture

### Backend
- In-memory BTreeMap library with virtual containers (Artists/Albums/Genres/All Tracks)
- No database for library, no transcoding — files served as-is
- `Arc<RwLock<Library>>` (parking_lot) — never hold read guard across await
- Object IDs: `ar{n}` artists, `aa{n}` artist-albums, `av{n}` album-view, `gr{n}` genres, `t{n}` tracks
- Playlists play through the session engine using the source ID `pl{n}` (not a library object)
- Two device communication channels:
  - **UPnP SOAP** (port varies per device): AVTransport, RenderingControl, PlayQueue — used for playback control and state queries
  - **HTTPS API** (port 443, self-signed certs): EQ, source switching, multiroom grouping, device status — Linkplay proprietary `httpapi.asp?command=...`
- SSDP discovery for WiiM MediaRenderer devices on the local network
- Server-side queue engine with Poweramp-style shuffle/repeat modes
- One server-wide playback session with group/track shuffle, gapless pre-fetch, auto-advance
- Enabled WiiMs are reconciled into one playing group; disabled WiiMs are detached and stopped
- SSE for real-time playback state + device change push to frontend
- SQLite for playlists and preferences; playlist writes accept container IDs and expand them to tracks
- Metadata tag editing via lofty (ID3/Vorbis), with bulk operations and library rescan
- Album art extraction with SQLite cache
- SOAP retry with exponential backoff for transient network errors

### Frontend
- Zustand stores, TanStack Query, Tailwind CSS
- Mobile-first layout: bottom nav, expandable now-playing bar
- SSE hook for real-time device/playback updates (no polling)
- Track metadata editor with single-track and bulk edit dialogs

## Key Constraints

- Server needs host networking for SSDP multicast device discovery
- Output membership is a persisted Airwave preference; physical multiroom state is read from the devices and reconciled to it
- Physical multiroom grouping uses HTTPS API, NOT SOAP (see `backend/docs/WIIM-PROTOCOL.md`)
- Music volume mounted read-write when metadata editing is enabled
- Public UI/API access is protected by the shared ALB Cognito rule. Keep DLNA,
  SOAP, and media streaming compatible with LAN devices; do not put Cognito in
  the backend request path that WiiM devices use.

## Style

- Server: minimal deps, quick-xml writer, SOAP faults with UPnP error codes, axum routers
- Frontend: functional components, typed API layer, no class components
