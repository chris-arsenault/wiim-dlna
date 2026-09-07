# Architecture

## Overview

Airwave implements a UPnP MediaServer:1 device. The collector advertises that
identity on the WiiM subnet; once discovered, players use Airwave's existing
HTTP description, SOAP browse, and streaming endpoints directly:

```
┌─────────────────────────────────────────────────────────────┐
│                     WiiM Device                             │
│                  (DLNA Media Renderer)                      │
└─────┬──────────────┬──────────────┬──────────────┬──────────┘
      │ 1. Discovery │ 2. Description │ 3. Browse   │ 4. Stream
      │ SSDP to      │    (HTTP/XML)  │  (SOAP/XML) │  (HTTP)
      │ collector    │                │             │
      ▼              ▼                ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│                     airwave                                │
│                                                              │
│  ┌──────────┐  ┌──────────────┐  ┌───────────┐  ┌────────┐ │
│  │Collector │  │  UPnP XML    │  │ Services  │  │ Stream │ │
│  │  lease   │  │ Descriptions │  │ (SOAP)    │  │ (HTTP) │ │
│  └──────────┘  └──────────────┘  └───────────┘  └────────┘ │
│         │              │               │              │      │
│         └──────────────┴───────┬───────┴──────────────┘      │
│                                │                             │
│                        ┌───────────────┐                     │
│                        │    Library    │                     │
│                        │  (in-memory)  │                     │
│                        └───────────────┘                     │
│                                │                             │
│                        ┌───────────────┐                     │
│                        │  Filesystem   │                     │
│                        │   Scanner     │                     │
│                        └───────────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

## Module Map

```
src/
├── main.rs                         Application bootstrap, HTTP routing
├── lib.rs                          Public re-exports for integration tests
├── config.rs                       TOML configuration with IP override
├── api.rs                          REST admin endpoints (/api/*)
├── streaming.rs                    HTTP Range-aware file serving
│
├── upnp/
│   ├── xml.rs                      Device + service description XML (device.xml, SCPDs)
│   ├── soap.rs                     SOAP envelope parsing and generation
│   └── didl.rs                     DIDL-Lite XML builder for Browse results
│
├── services/
│   ├── content_directory.rs        Browse, GetSearchCapabilities, GetSortCapabilities
│   └── connection_manager.rs       GetProtocolInfo, GetCurrentConnectionIDs
│
└── media/
    ├── library.rs                  In-memory tree + filesystem scanner
    └── metadata.rs                 Audio tag extraction (lofty)
```

## Request Flow

### 1. Discovery (SSDP)

```
Airwave:     PUT collector /wiim/media-server (renewable lease)
Collector:   advertises five MediaServer targets on the IoT LAN
WiiM:        M-SEARCH → collector response
             LOCATION: http://192.168.66.3:7882/device.xml
```

Airwave refreshes the lease every ten minutes. The collector expires it when
Airwave stops refreshing, sends alive messages, and answers searches with the
same UUID and HTTP location. Airwave opens no UDP socket.

### 2. Description (HTTP/XML)

```
WiiM fetches:  GET /device.xml
Server:        upnp::xml::device_description()
               → returns XML with UDN, friendly name, service list

WiiM fetches:  GET /ContentDirectory.xml
Server:        upnp::xml::content_directory_scpd()
               → returns SCPD with Browse action definition
```

### 3. Control (SOAP)

```
WiiM sends:  POST /control/ContentDirectory
             SOAPAction: "urn:schemas-upnp-org:service:ContentDirectory:1#Browse"
             Body: <Browse><ObjectID>0</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag>...</Browse>

Server:      main::handle_soap_control()
             → upnp::soap::parse_soap_action()       (extract action + args)
             → services::content_directory::handle_browse()
             → media::library::Library::children_of() (get objects)
             → upnp::didl::DidlWriter                 (serialize to DIDL-Lite XML)
             → upnp::soap::soap_response()            (wrap in SOAP envelope)
```

### 4. Streaming (HTTP)

```
WiiM sends:  GET /media/t42
             Range: bytes=0-

Server:      main::stream_media()
             → library.get("t42")        (look up track path)
             → streaming::serve_file()   (open file, handle Range, stream)
             → HTTP 206 with Content-Range, DLNA headers
```

## Data Model

```
Library (in-memory, Arc<RwLock<Library>>)
│
├── "0" Container (Root)
│   ├── "a1" Container (Artist: "Pink Floyd")
│   │   ├── "al1" Container (Album: "The Wall")
│   │   │   ├── "t1" Track → /mnt/music/Pink Floyd/The Wall/01 - In The Flesh.flac
│   │   │   ├── "t2" Track → /mnt/music/Pink Floyd/The Wall/02 - The Thin Ice.flac
│   │   │   └── ...
│   │   └── "al2" Container (Album: "Wish You Were Here")
│   │       └── ...
│   ├── "a2" Container (Artist: "Miles Davis")
│   │   └── ...
│   └── ...
```

IDs are prefixed by type: `a` = artist, `al` = album, `t` = track. Root is always `"0"`.

The library is rebuilt from scratch on each scan (no incremental updates). This is intentional — a full rescan of 100k tracks takes ~2 seconds, and the atomic swap via `RwLock` means zero downtime for clients during rescans.

## Concurrency Model

```
main thread
├── axum HTTP server (tokio, multi-threaded)
├── tokio::spawn → collector registration renewal
├── tokio::spawn → collector inventory + output reconciliation
├── tokio::spawn → playback monitor
└── tokio::spawn → library::scan_loop()  (periodic rescan)
```

The library is shared via `Arc<RwLock<Library>>` (parking_lot). Read locks are held only briefly during Browse/stream lookups. Write locks only during scan completion (atomic swap).

Collector failures leave the existing device registry intact for that polling
cycle. A device is removed only after a successful inventory response marks it
unreachable or omits it.

## Key Design Decisions

**No database.** The entire media library is an in-memory BTreeMap. Music metadata for 100k tracks fits in ~50 MB. Startup scan is fast. No schema migrations, no corruption, no backup concerns.

**No eventing.** UPnP eventing (SUBSCRIBE/NOTIFY for state changes) is not implemented. WiiM devices work fine without it — they poll Browse on navigation. This removes significant complexity.

**No transcoding.** Files are served as-is. WiiM hardware supports all common lossless and lossy formats natively.

**Deterministic UUID.** The device UUID is derived from the friendly name via UUID v5 (SHA-1 namespace). This means the same server always appears as the same device to WiiM, even across container rebuilds — no duplicate entries in the WiiM app.

**parking_lot over std.** `parking_lot::RwLock` is used because `std::sync::RwLock` guards are not `Send`, which conflicts with tokio's work-stealing scheduler. parking_lot guards have the same restriction but the code is structured to never hold a guard across an await point.

## Dependencies

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime |
| axum | HTTP server |
| quick-xml | XML generation/parsing for SOAP and DIDL |
| lofty | Audio metadata extraction (ID3, Vorbis, FLAC) |
| serde + toml | Configuration |
| socket2 | Explicit HTTP listener socket setup |
| parking_lot | Fast RwLock |
| walkdir | Recursive directory traversal |
| uuid | Deterministic device UUID |
| mime_guess | MIME type detection from file extensions |
| percent-encoding | URL encoding for track IDs |
| tokio-util | ReaderStream for async file streaming |
| local-ip-address | Auto-detect host IP |
| tracing | Structured logging |

## Control Plane

In addition to serving media, airwave-server acts as the control plane for WiiM devices on the network. This adds several module groups not shown in the DLNA-focused diagram above:

```
src/
├── control/
│   ├── mod.rs                      Control plane routes + state
│   ├── state.rs                    Shared state (DeviceManager, EventBus, sessions)
│   ├── outputs.rs                  WiiM output membership + physical group reconciliation
│   ├── eq.rs                       EQ, balance, crossfade, source switching, WiFi status
│   ├── playback_monitor.rs         Polling of the singleton playback transport
│   ├── session.rs                  Singleton playback engine (shuffle/repeat/gapless)
│   ├── events.rs                   SSE event bus for real-time frontend push
│   ├── device_config.rs            SQLite persistence for output + application preferences
│   └── models.rs                   Request/response types
│
├── wiim/
│   ├── collector.rs                Authenticated inventory, probe, and MediaServer lease client
│   ├── discovery.rs                Inventory registration + device-owned group state refresh
│   ├── device.rs                   WiimDevice model + DeviceManager (DashMap)
│   ├── https_api.rs                LinkPlay semantics over the collector route
│   ├── soap_client.rs              SOAP semantics over collector routes, with retry
│   └── services/
│       ├── av_transport.rs         AVTransport:1 (play, pause, seek, GetInfoEx)
│       ├── rendering_control.rs    RenderingControl:1 (volume, mute, GetControlDeviceInfo)
│       └── play_queue.rs           PlayQueue:1 (WiiM-proprietary queue)
```

WiiM devices expose two distinct APIs. Airwave still builds commands and parses
their responses; the collector resolves the device ID to its on-link endpoints
and forwards bytes. See [docs/WIIM-PROTOCOL.md](docs/WIIM-PROTOCOL.md) for the
protocol reference including multiroom grouping, EQ, source switching, and
known idiosyncrasies.

The control API exposes one logical playback target, `playing`, with one queue,
session, timer, and library-navigation state. Only WiiMs enter the device
registry. Airwave persists each WiiM's desired on/off output membership,
forms one physical group from the enabled WiiMs, and detaches and stops disabled
WiiMs without changing their power or mute settings. Each WiiM also has a
persisted base level. The main player's persisted volume is a global multiplier,
so the physical level is `base level × main volume` and relative speaker balance
survives main-volume changes. Airwave writes effective levels through each
device's direct Linkplay API; it never uses group-propagating SOAP volume writes.
An output receives its effective level before it joins the playing group. Output
changes run as serialized background transitions: desired membership is
persisted immediately, each Linkplay command is sent once, and hardware
convergence must match twice five seconds apart within a 90-second phase.
Routine follower joins do not reload the playing URI. Only moving playback away
from a disabled group master transfers the URI and position to a new master.

A failed direct transition escalates once to a different operation: reset every
present WiiM to standalone, software-stop desired-off outputs, and rebuild the
whole desired group. A second failure persists a global recovery-required latch.
Discovery and timers do not retry it, playback commands are rejected while the
topology is indeterminate, and Airwave makes one best-effort software stop on
every WiiM so an inseparable unwanted output cannot continue making noise.
Toggle changes then only edit desired membership. The
`POST /api/outputs/recover` action starts one new recovery epoch explicitly;
`GET /api/outputs` and `output_state_changed` expose its state to the frontend.
Every nonmatching topology sample is logged with the observed roles so a later
timeout identifies the exact disagreement.

## Protocol References

- [UPnP Device Architecture 1.0](http://upnp.org/specs/arch/UPnP-arch-DeviceArchitecture-v1.0.pdf)
- [ContentDirectory:1 Service](http://upnp.org/specs/av/UPnP-av-ContentDirectory-v1-Service.pdf)
- [ConnectionManager:1 Service](http://upnp.org/specs/av/UPnP-av-ConnectionManager-v1-Service.pdf)
- [DLNA Guidelines](https://spirespark.com/dlna/guidelines) (protocol info flags, ORG_OP, ORG_FLAGS)
- [DIDL-Lite Schema](http://www.upnp.org/schemas/av/didl-lite-v2.xsd)
- [WiiM Protocol Reference](docs/WIIM-PROTOCOL.md) (reverse-engineered Linkplay API, multiroom, EQ, idiosyncrasies)
