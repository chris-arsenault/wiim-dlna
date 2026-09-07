mod api;
mod config;
mod control;
mod media;
mod services;
mod streaming;
mod upnp;
mod wiim;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    config: config::Config,
    library: media::library::SharedLibrary,
    uuid: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "airwave_server=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    let cfg = match config::Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config from {config_path}: {e}");
            std::process::exit(1);
        }
    };
    let collector_token = match config::Config::collector_token() {
        Ok(token) => token,
        Err(error) => {
            error!("{error}");
            std::process::exit(1);
        }
    };
    let collector = wiim::collector::CollectorClient::new(&cfg.collector.url, collector_token);

    info!(
        "Starting {} on {}:{} with WiiM collector {}",
        cfg.server.friendly_name,
        cfg.effective_ip(),
        cfg.network.port,
        cfg.collector.url,
    );

    // Generate a stable UUID based on friendly name (deterministic across restarts)
    let uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_DNS,
        cfg.server.friendly_name.as_bytes(),
    )
    .to_string();

    let library = media::library::new_shared();

    // Initial scan
    {
        let dirs = cfg.media.music_dirs.clone();
        let lib = library.clone();
        let new_lib = tokio::task::spawn_blocking(move || media::library::scan(&dirs))
            .await
            .expect("initial scan panicked");
        *lib.write() = new_lib;
    }

    let state = AppState {
        config: cfg.clone(),
        library: library.clone(),
        uuid: uuid.clone(),
    };

    let api_state = api::ApiState {
        config: cfg.clone(),
        library: library.clone(),
    };

    let api_router = Router::new()
        .route("/status", get(api::get_status))
        .route("/config", get(api::get_config))
        .route("/rescan", post(api::rescan))
        .with_state(api_state);

    // Ensure data directory exists
    let data_dir = &cfg.server.data_dir;
    std::fs::create_dir_all(data_dir).expect("Failed to create data directory");

    // Control API state
    let device_manager = Arc::new(wiim::device::DeviceManager::new());
    let event_bus = control::events::EventBus::new();
    let device_config_db = data_dir.join("device_config.db");
    let device_config_store = Arc::new(control::device_config::DeviceConfigStore::new(
        device_config_db.to_str().unwrap_or("device_config.db"),
    ));
    let playlist_db = data_dir.join("playlists.db");
    let playlist_store = Arc::new(control::playlists::PlaylistStore::new(
        playlist_db.to_str().unwrap_or("playlists.db"),
    ));
    let queue_manager = Arc::new(control::queue::QueueManager::new());
    let session_manager = Arc::new(control::session::SessionManager::new());
    let art_cache = Arc::new(media::art::ArtCache::new(data_dir));

    let sleep_timer_manager = control::timer::SleepTimerManager::new();
    let collector_ready = Arc::new(AtomicBool::new(false));
    let global_volume = control::volume::load_global_volume(&device_config_store);
    let output_recovery = control::outputs::load_recovery_state(&device_config_store);

    let control_state = control::state::ControlState {
        devices: device_manager.clone(),
        device_config: device_config_store.clone(),
        library: library.clone(),
        events: event_bus.clone(),
        playlists: playlist_store,
        queues: queue_manager.clone(),
        sessions: session_manager.clone(),
        art_cache,
        sleep_timers: sleep_timer_manager,
        output_lock: Arc::new(tokio::sync::Mutex::new(())),
        output_recovery: Arc::new(parking_lot::RwLock::new(output_recovery)),
        volume_lock: Arc::new(tokio::sync::Mutex::new(())),
        global_volume: Arc::new(parking_lot::RwLock::new(global_volume)),
        base_url: cfg.base_url(),
        collector_ready: Arc::clone(&collector_ready),
    };

    // Routes match frontend API client paths (relative to /api/)
    let control_router = Router::new()
        // Health
        .route("/health", get(control::health::health))
        // SSE events
        .route("/events", get(control::events::sse_handler))
        // Devices
        .route("/devices", get(control::devices::list_devices))
        .route("/devices/{id}", get(control::devices::get_device))
        .route(
            "/library/state",
            get(control::devices::get_library_state).post(control::devices::set_library_state),
        )
        .route("/devices/{id}/volume", post(control::devices::set_volume))
        .route("/devices/{id}/mute", post(control::devices::toggle_mute))
        .route(
            "/devices/{id}/enabled",
            post(control::outputs::set_output_enabled),
        )
        .route(
            "/outputs",
            get(control::outputs::get_output_state),
        )
        .route(
            "/outputs/recover",
            post(control::outputs::recover_outputs),
        )
        .route("/devices/{id}/name", post(control::devices::rename_device))
        .route(
            "/devices/{id}/channel",
            get(control::devices::get_channel).post(control::devices::set_channel),
        )
        .route(
            "/playback/sleep-timer",
            get(control::timer::get_sleep_timer)
                .post(control::timer::set_sleep_timer)
                .delete(control::timer::cancel_sleep_timer),
        )
        .route(
            "/devices/{id}/source",
            post(control::eq::set_source),
        )
        .route(
            "/devices/{id}/wifi",
            get(control::eq::get_wifi_status),
        )
        // One global playback object; physical WiiM output selection is internal.
        .route("/playback", get(control::playback::get_state))
        .route("/playback/play", post(control::playback::play))
        .route("/playback/stop", post(control::playback::stop))
        .route("/playback/pause", post(control::playback::pause))
        .route("/playback/resume", post(control::playback::resume))
        .route("/playback/volume", post(control::playback::set_volume))
        .route("/playback/next", post(control::playback::next_track))
        .route("/playback/prev", post(control::playback::prev_track))
        .route("/playback/seek", post(control::playback::seek))
        .route("/playback/seek-forward", post(control::playback::seek_forward))
        .route("/playback/seek-backward", post(control::playback::seek_backward))
        .route("/playback/rate", post(control::playback::rate_track))
        .route("/playback/shuffle", post(control::playback::set_shuffle))
        .route("/playback/repeat", post(control::playback::set_repeat))
        // Session-based playback
        .route(
            "/playback/session/play",
            post(control::playback::session_play),
        )
        .route(
            "/playback/session/next",
            post(control::playback::session_next),
        )
        .route(
            "/playback/session/prev",
            post(control::playback::session_prev),
        )
        .route(
            "/playback/session/shuffle",
            post(control::playback::session_set_shuffle),
        )
        .route(
            "/playback/session/repeat",
            post(control::playback::session_set_repeat),
        )
        // Queue belongs to the global playback object.
        .route(
            "/playback/queue",
            get(control::playback::get_queue).delete(control::playback::clear_queue),
        )
        .route(
            "/playback/queue/add",
            post(control::playback::add_to_queue),
        )
        .route(
            "/playback/queue/move",
            post(control::playback::move_in_queue),
        )
        .route(
            "/playback/queue/{index}",
            delete(control::playback::remove_from_queue),
        )
        // EQ (HTTPS API-based)
        .route("/eq/{id}/state", get(control::eq::get_eq_state))
        .route("/eq/{id}/presets", get(control::eq::get_eq_presets))
        .route(
            "/eq/{id}/presets/{name}",
            delete(control::eq::delete_eq_preset),
        )
        .route("/eq/{id}/load", post(control::eq::load_eq_preset))
        .route("/eq/{id}/enable", post(control::eq::enable_eq))
        .route("/eq/{id}/disable", post(control::eq::disable_eq))
        .route("/eq/{id}/band", post(control::eq::set_eq_band))
        .route("/eq/{id}/save", post(control::eq::save_eq_preset))
        .route(
            "/eq/{id}/balance",
            get(control::eq::get_balance).post(control::eq::set_balance),
        )
        .route(
            "/eq/{id}/crossfade",
            get(control::eq::get_crossfade).post(control::eq::set_crossfade),
        )
        // Playlists
        .route(
            "/playlists",
            get(control::playlists::list_playlists).post(control::playlists::create_playlist),
        )
        .route(
            "/playlists/{id}",
            get(control::playlists::get_playlist).delete(control::playlists::delete_playlist),
        )
        .route(
            "/playlists/{id}/tracks",
            post(control::playlists::add_playlist_tracks),
        )
        .route(
            "/playlists/{id}/tracks/{position}",
            delete(control::playlists::remove_playlist_track),
        )
        // Art
        .route("/art/{id}", get(control::art::get_art))
        // Library
        .route("/library/browse", get(control::library::browse))
        .route("/library/search", get(control::library::search))
        // Metadata editing
        .route(
            "/library/tracks/{id}",
            patch(control::metadata::update_track),
        )
        .route(
            "/library/bulk/album-artist",
            post(control::metadata::bulk_set_album_artist),
        )
        .route(
            "/library/bulk/rename-artist",
            post(control::metadata::bulk_rename_artist),
        )
        .with_state(control_state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
        );

    let app = Router::new()
        // UPnP device/service descriptions
        .route("/device.xml", get(device_description))
        .route("/ContentDirectory.xml", get(content_directory_scpd))
        .route("/ConnectionManager.xml", get(connection_manager_scpd))
        // SOAP control endpoints
        .route("/control/ContentDirectory", post(control_content_directory))
        .route(
            "/control/ConnectionManager",
            post(control_connection_manager),
        )
        // Event subscription (minimal stub)
        .route("/event/ContentDirectory", get(event_stub))
        .route("/event/ConnectionManager", get(event_stub))
        // Media streaming
        .route("/media/{id}", get(stream_media))
        // Control API (routes match frontend expectations)
        .nest("/api", control_router)
        // Admin/config API (separate prefix to avoid nest collision)
        .nest("/admin", api_router)
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let bind_addr = SocketAddr::from(([0, 0, 0, 0], cfg.network.port));
    let listener = {
        let socket = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::STREAM, None)
            .expect("failed to create TCP socket");
        socket.set_reuse_address(true).expect("SO_REUSEADDR");
        socket.set_nonblocking(true).expect("set_nonblocking");
        socket
            .bind(&socket2::SockAddr::from(bind_addr))
            .expect("failed to bind TCP socket");
        socket.listen(1024).expect("TCP listen");
        tokio::net::TcpListener::from_std(socket.into()).expect("tokio TcpListener from std")
    };
    info!("HTTP server listening on {bind_addr}");

    // Start background tasks

    // Library periodic rescan
    let scan_lib = library.clone();
    let scan_dirs = cfg.media.music_dirs.clone();
    let scan_interval = cfg.media.scan_interval_secs;
    tokio::spawn(media::library::scan_loop(
        scan_lib,
        scan_dirs,
        scan_interval,
    ));

    // The collector advertises this existing MediaServer locally on the IoT
    // LAN. Registration is leased so a stopped Airwave disappears naturally.
    tokio::spawn(wiim::collector::run_media_registration(
        collector.clone(),
        uuid.clone(),
        cfg.base_url(),
    ));

    // Collector inventory supplies renderer reachability; Airwave retains
    // WiiM filtering, output reconciliation, and every playback semantic.
    tokio::spawn(wiim::discovery::run_discovery(
        control_state.clone(),
        collector,
        Duration::from_secs(30),
    ));

    // Playback monitor (auto-advance session/queue on track end)
    tokio::spawn(control::playback_monitor::run_playback_monitor(
        control_state.clone(),
    ));

    axum::serve(listener, app)
        .await
        .expect("HTTP server failed");
}

async fn device_description(State(state): State<AppState>) -> impl IntoResponse {
    let xml = upnp::xml::device_description(
        &state.uuid,
        &state.config.server.friendly_name,
        &state.config.base_url(),
    );
    Response::builder()
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(Body::from(xml))
        .unwrap()
}

async fn content_directory_scpd() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(Body::from(upnp::xml::content_directory_scpd()))
        .unwrap()
}

async fn connection_manager_scpd() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(Body::from(upnp::xml::connection_manager_scpd()))
        .unwrap()
}

async fn control_content_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    handle_soap_control(&headers, &body, |action| {
        services::content_directory::handle_action(action, &state.library, &state.config.base_url())
    })
}

async fn control_connection_manager(
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    handle_soap_control(&headers, &body, |action| {
        services::connection_manager::handle_action(action)
    })
}

fn handle_soap_control(
    headers: &HeaderMap,
    body: &[u8],
    handler: impl FnOnce(&upnp::soap::SoapAction) -> Result<(String, u16), (String, u16)>,
) -> Response {
    let soap_action = headers
        .get("SOAPAction")
        .or_else(|| headers.get("soapaction"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    match upnp::soap::parse_soap_action(soap_action, body) {
        Ok(action) => {
            let (xml, status) = match handler(&action) {
                Ok(r) => r,
                Err(r) => r,
            };
            Response::builder()
                .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
                .body(Body::from(xml))
                .unwrap()
        }
        Err(e) => {
            let fault = upnp::soap::soap_fault("s:Client", "UPnPError", 401, &e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
                .body(Body::from(fault))
                .unwrap()
        }
    }
}

async fn event_stub() -> impl IntoResponse {
    StatusCode::OK
}

async fn stream_media(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let path = {
        let lib = state.library.read();
        match lib.get(&id) {
            Some(media::library::LibraryObject::Track(track)) => track.path.clone(),
            _ => return StatusCode::NOT_FOUND.into_response(),
        }
    };
    streaming::serve_file(&path, &headers).await
}
