# WiiM Device Protocol Reference

Protocol details for WiiM Mini (Linkplay-based) devices. These devices expose three network interfaces, each serving different functions. Most behavior below was established against the devices directly; the recovery commands are additionally checked against Linkplay's published router-mode API.

Tested on: WiiM Mini, firmware `Linkplay.4.6.805929`, hardware `ALLWINNER-R328`.

## Communication Channels

| Port | Protocol | Purpose |
|------|----------|---------|
| Varies (e.g. 59152) | HTTP (UPnP SOAP) | Playback control, volume, state queries |
| 443 | HTTPS (self-signed) | EQ, source switching, multiroom grouping, extended status |
| 5356 | HTTP | Spotify Connect eSDK (group aliases, device info) |

The SOAP port is discovered from the device's SSDP response `LOCATION` header, which points to a `description.xml`. The port is NOT fixed — it varies per device and firmware version (49152 and 59152 have both been observed).

### SOAP (UPnP)

Standard UPnP SOAP calls to the device's MediaRenderer services. Our code uses `SoapClient` with service-specific wrappers.

**Services:**
- `RenderingControl:1` — volume, mute, channel, `GetControlDeviceInfo` (WiiM extension)
- `AVTransport:1` — playback state, transport control, `GetInfoEx` (WiiM extension)
- `PlayQueue:1` (`urn:schemas-wiimu-com`) — WiiM-proprietary queue management

**Key WiiM SOAP extensions:**
- `GetControlDeviceInfo` — returns `MultiType`, `SlaveList` (JSON), `Status` (JSON with device name, firmware, group state, UUID, WiFi info)
- `GetInfoEx` — returns extended transport info including `SlaveFlag`, `MasterUUID`, `SlaveList`, `SubNum`

### HTTPS API (Linkplay)

All commands use the same endpoint: `GET https://{ip}/httpapi.asp?command={command}`

The device uses a self-signed TLS certificate. Responses are either plain text (`OK`, `unknown command`) or JSON.

**Source code:** `wiim-server/src/wiim/https_api.rs`

### eSDK (Port 5356)

Spotify Connect endpoint. Returns JSON with device info, Spotify auth state, and group aliases. Not used by our server currently but useful for debugging.

```
GET http://{ip}:5356/zc?action=getInfo
```

## Multiroom Grouping

**Critical:** Grouping MUST use the HTTPS API. The SOAP `MultiRoomJoinGroup`/`MultiRoomLeaveGroup` commands return `OK` but do NOT actually form groups on WiiM Mini.

### Creating a Group

Send to each **slave** device:

```
GET https://{slave_ip}/httpapi.asp?command=ConnectMasterAp:JoinGroupMaster:eth{master_ip}:wifi0.0.0.0
→ OK
```

The `eth` and `wifi` prefixes are part of the Linkplay protocol. Airwave uses
the documented router-mode form: the master address follows `eth`, and the
unused peer address is `0.0.0.0`. The previous payload put the master address
in both fields; a device could acknowledge it without changing topology.

An `OK` response acknowledges the command; it does not mean the multiroom
topology has converged. WiiM hardware can take 30–60 seconds to finish a join
or detach. Airwave therefore sends each topology command once, polls
`GetInfoEx` for up to 90 seconds per phase, and requires two complete matching
topology observations five seconds apart. Any mismatch or read failure resets
that stability count. Airwave rejects overlapping output changes while that
observation is pending.

A direct transition timeout starts one bounded recovery epoch; it does not
repeat the failed delta. Recovery resets every present WiiM to standalone,
stops outputs desired off, and builds the complete desired group once. If that
also fails, Airwave persists a recovery-required latch and sends no more group
commands until the user explicitly requests another recovery. It also makes one
best-effort software stop on every WiiM: if the hardware cannot separate an off
speaker from the playing group, Airwave fails silent instead of leaving that
speaker attached to an active stream.

### Dissolving a Group

To remove one follower while retaining the rest of the group, send to the
master:

```
GET https://{master_ip}/httpapi.asp?command=multiroom:SlaveKickout:{follower_ip}
→ OK
```

Send once to the **master** device to split the complete group:

```
GET https://{master_ip}/httpapi.asp?command=multiroom:Ungroup
→ OK
```

During recovery Airwave sends this once to every present WiiM because a stale
device may disagree with Airwave about which unit is the master. If the
subsequent topology sample still identifies a follower, Airwave sends that
follower one local standalone command:

```
GET https://{follower_ip}/httpapi.asp?command=ConnectMasterAp:JoinGroupMaster:eth0
→ OK
```

Both stages are bounded. Observed topology decides whether they succeeded;
`OK` alone never does.

### Querying Group State

**From the master** (HTTPS API):

```
GET https://{master_ip}/httpapi.asp?command=multiroom:getSlaveList
→ { "slaves": 1, "wmrm_version": "4.2", "slave_list": [
    { "name": "WiiM - Gym", "uuid": "FF9700164482267319D4D9E8",
      "ip": "192.0.2.21", "version": "4.2", ... }
  ]}
```

**From any device** (SOAP `GetControlDeviceInfo`):
- Master: `SlaveList` JSON has `"slaves": N` where N > 0
- Slave: Status JSON has `"group": "1"` (vs `"0"` when ungrouped)

**From slave** (SOAP `GetInfoEx`):
- `SlaveFlag`: `"1"` when slaved
- `MasterUUID`: hex UUID of the master device (e.g. `FF97001611F40A599E9E3551`)

### Group State is Device-Canonical

The device is the single source of truth for observed group state. Other apps
(WiiM app, Spotify) can create or dissolve groups at any time. Airwave reads
group state during discovery, but suppresses those intermediate observations
while one of its own topology transitions is pending. Airwave persists desired
on/off output membership immediately, separately from the physical group fields.
After a recovery failure, later toggle changes update that desired state but do
not operate the speakers. The explicit recovery action consumes the latest
desired set.

### WiiM App vs Our Grouping

The WiiM app also creates groups via the HTTPS API (confirmed via pcap analysis). It additionally communicates with `api.linkplay.com` for cloud features, but the local HTTPS API commands are sufficient for multiroom.

The eSDK endpoint (port 5356) reflects a separate concept: Spotify Connect group aliases. These are NOT the same as multiroom groups — a device can have a Spotify group alias while being ungrouped at the multiroom level.

## EQ and Audio Settings (HTTPS API)

All require `capabilities.https_api = true`.

| Command | Description |
|---------|-------------|
| `EQGetList` | List preset names (JSON array) |
| `EQGetBand` | Current EQ state + band values |
| `EQLoad:{name}` | Load a preset by name |
| `EQOn` / `EQOff` | Enable/disable EQ |
| `EQSetBand:{json}` | Set a single band: `{"index": N, "value": V}` |
| `EQSave:{name}` | Save current bands as a named preset |
| `EQDel:{name}` | Delete a saved preset |
| `getChannelBalance` | Returns balance as float |
| `setChannelBalance:{value}` | Set L/R balance |
| `GetFadeFeature` | Crossfade state: `{"FadeFeature": 0\|1}` |
| `SetFadeFeature:0\|1` | Enable/disable crossfade |

## Source Switching (HTTPS API)

```
GET https://{ip}/httpapi.asp?command=setPlayerCmd:switchmode:{source}
→ OK
```

Known source values: `wifi`, `bluetooth`, `line-in`, `optical`, `coaxial`, `udisk`, `HDMI`, `RCA`

## Device Status (HTTPS API)

```
GET https://{ip}/httpapi.asp?command=getStatusEx
```

Returns a large JSON object. Key fields:

| Field | Description |
|-------|-------------|
| `uuid` | Device UUID (hex, no dashes) |
| `DeviceName` | User-facing device name |
| `GroupName` | Usually same as DeviceName when ungrouped |
| `group` | `"0"` = ungrouped, `"1"` = in a group as slave |
| `firmware` | e.g. `Linkplay.4.6.805929` |
| `RSSI` | WiFi signal strength in dBm (string) |
| `essid` | WiFi SSID (hex-encoded) |
| `apcli0` | Device IP address |
| `wmrm_version` | Multiroom protocol version |
| `communication_port` | `8819` (purpose unknown, appears non-responsive) |
| `security` | `https/2.0` |

## Volume in Groups

**SOAP `SetVolume` on a master device syncs volume to all slaves** (firmware behavior). This causes crosstalk — setting the master's volume drags every slave with it.

The HTTPS API does NOT have this behavior:

| Method | Syncs to group? |
|--------|----------------|
| SOAP `SetVolume` on master | **YES** — firmware pushes to all slaves |
| SOAP `SetVolume` on slave | No — slave changes independently |
| HTTPS `setPlayerCmd:vol:{N}` on master | No — master only |
| HTTPS `setPlayerCmd:vol:{N}` on slave | No — slave only |
| HTTPS `multiroom:SlaveVolume:{ip}:{vol}` | No — sets specific slave only |

Airwave admits only WiiM renderers and uses `setPlayerCmd:vol` for every volume
write. It does not fall back to SOAP because doing so could flatten the group.
Each device has a persisted base level, while the main player volume is a
persisted multiplier. Airwave writes `round(base × main × 100)` to each enabled
device. A newly enabled device receives that effective level before joining the
playing group.

The `multiroom:getSlaveList` response includes each slave's current volume and mute state.

## Known Idiosyncrasies

1. **SOAP multiroom commands are no-ops on WiiM Mini.** `MultiRoomJoinGroup` and `MultiRoomLeaveGroup` return success but have no effect. Always use HTTPS API.

2. **SOAP port varies.** Don't hardcode it. Discover from SSDP response → `description.xml`.

3. **Self-signed TLS on port 443.** Must use `danger_accept_invalid_certs(true)` or equivalent. No CA chain.

4. **`group` field semantics differ between master and slave.** The master's `getStatusEx` shows `group: 0` even when it HAS slaves. Only the slave sets `group: 1`. To detect a master, check `multiroom:getSlaveList` or SOAP `SlaveList`.

5. **`GroupName` is misleading.** It's usually just the device name, not an actual group name. It does NOT change when the device joins a group.

6. **eSDK group aliases are Spotify-only.** The `aliases` array from port 5356 `/zc?action=getInfo` shows Spotify Connect groups, not multiroom groups. A device can show `isGroup: true` in aliases while being ungrouped at the multiroom level.

7. **WiiM app uses cloud API.** The WiiM app communicates with `api.linkplay.com` and `ota-app.linkplay.com` for account features. Local grouping does NOT require cloud access.

8. **Status JSON values are strings.** Numeric fields like `group`, `RSSI`, and `volume_control` come back as strings in the JSON, not numbers. The `essid` (SSID) is hex-encoded.

9. **Discovery can miss the HTTPS API.** If the HTTPS endpoint is slow to start after a reboot, the initial probe may fail. The `capabilities.https_api` flag is set at discovery time and not re-probed. A device restart may require a server restart to re-detect HTTPS.
