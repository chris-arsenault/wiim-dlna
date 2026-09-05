# Deployment model

Airwave deliberately has separate LAN and public delivery paths because WiiM
discovery and streaming must remain local while public control requires an
identity boundary.

## LAN

Docker Compose deploys two host-networked containers to TrueNAS through Komodo:

- `airwave-backend` listens on port 7882 for UPnP/SOAP, media streaming, and
  the control API. It does not open an SSDP socket or connect to WiiM addresses.
- `airwave-frontend` listens on port 7880 and proxies `/api/*` to
  `127.0.0.1:7882`.

The container frontend's runtime configuration has `authRequired: false`, so
`http://<server>:7880` is usable without Cognito. This does not add
authentication to the backend's direct LAN endpoints; network access remains
the LAN trust boundary.

The backend authenticates to
`https://collector.local.ahara.io:8443` with the secret mapped from
`/ahara/airwave/collector/api-token`. It polls native renderer inventory and
uses only the collector's device-ID routes for UPnP and LinkPlay commands.
Airwave still parses every response and owns the singleton playback state,
desired output membership, physical WiiM grouping, and EQ.

Airwave renews a lease for its existing MediaServer identity. The collector
answers WiiM SSDP searches and sends the five UPnP advertisements on the IoT
LAN. Their `LOCATION` points to `http://192.168.66.3:7882/device.xml`, so WiiMs
fetch descriptions, browse the library, and stream media directly from
Airwave over the one declared TCP 7882 flow. No UDP crosses the VLAN boundary.
`GET /api/health` reports `collector.connected`; logs record inventory and
lease-renewal failures without printing the bearer token.

## Public web and API

Terraform deploys the Vite build to an encrypted private S3 bucket behind
CloudFront at `airwave.ahara.io`. Its generated runtime configuration enables
Cognito and points API calls to `https://api.airwave.ahara.io`.

The API hostname terminates at the shared AWS ALB. All non-OPTIONS `/api/*`
requests pass through Cognito issuer/JWKS JWT validation before the ALB forwards
them through the shared reverse proxy to the TrueNAS backend. The Rust server
does not duplicate that public-edge validation.

## Android

CI publishes the native widget to the shared F-Droid repository. It supports
both deployment paths:

- LAN server URL with Cognito disabled.
- Public API URL with native Cognito password, software-token MFA, encrypted
  refresh-token storage, and automatic ID-token refresh.

## CloudFront versus reverse-proxy hosting

CloudFront is retained for the public UI.

Moving the public UI to the TrueNAS reverse proxy would remove the S3/CloudFront
resources and make one container image the UI artifact everywhere. It would
also make the public UI dependent on the home server and reverse proxy, move
static asset delivery away from a private S3 origin/CDN, and require the same
frontend container to serve different authentication configuration by request
hostname. Serving its current unauthenticated LAN `config.js` publicly would
leave the public UI unable to obtain a Cognito JWT.

A safe migration therefore needs all of the following before removing the
Terraform website module:

1. Host-aware runtime configuration so the LAN host remains unauthenticated and
   the public host enables Cognito.
2. A reverse-proxy upstream for port 7880 plus an ALB listener rule,
   certificate, and DNS cutover for `airwave.ahara.io`.
3. Validation that public UI availability may follow TrueNAS/reverse-proxy
   availability.
4. Removal of the S3/CloudFront resources only after the new hostname is live.

The LAN requirement no longer depends on that migration, so retaining
CloudFront keeps the public path isolated without blocking local access.
