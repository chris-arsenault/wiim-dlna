use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use super::models::{
    ChannelRequest, DeviceCapabilitiesResponse, DeviceNameRequest, DeviceResponse,
    LibraryStateRequest, LibraryStateResponse, VolumeRequest,
};
use super::state::ControlState;

fn device_to_response(d: &crate::wiim::device::WiimDevice) -> DeviceResponse {
    DeviceResponse {
        id: d.id.clone(),
        name: d.name.clone(),
        ip: d.ip.clone(),
        model: d.model.clone(),
        firmware: d.firmware.clone(),
        device_type: d.device_type.clone(),
        enabled: d.enabled,
        capabilities: DeviceCapabilitiesResponse {
            av_transport: d.capabilities.av_transport,
            rendering_control: d.capabilities.rendering_control,
            wiim_extended: d.capabilities.wiim_extended,
            https_api: d.capabilities.https_api,
        },
        volume: d.volume,
        muted: d.muted,
        channel: d.channel.clone(),
        source: d.source.clone(),
        group_id: d.group_id.clone(),
        is_master: d.is_master,
    }
}

pub async fn list_devices(State(state): State<ControlState>) -> Json<Vec<DeviceResponse>> {
    let mut devices = state.devices.list_all();
    devices.retain(|device| device.device_type == "wiim");
    devices.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Json(devices.iter().map(device_to_response).collect())
}

pub async fn get_device(
    State(state): State<ControlState>,
    Path(id): Path<String>,
) -> Result<Json<DeviceResponse>, StatusCode> {
    let d = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if d.device_type != "wiim" {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(device_to_response(&d)))
}

fn default_library_path() -> Vec<super::models::LibraryPathEntry> {
    vec![super::models::LibraryPathEntry {
        id: "0".to_string(),
        title: "Library".to_string(),
    }]
}

pub async fn get_library_state(State(state): State<ControlState>) -> Json<LibraryStateResponse> {
    let path = state
        .device_config
        .load_app_state("library_path")
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_else(default_library_path);
    Json(LibraryStateResponse { path })
}

pub async fn set_library_state(
    State(state): State<ControlState>,
    Json(body): Json<LibraryStateRequest>,
) -> Result<StatusCode, StatusCode> {
    let path = if body.path.is_empty() {
        default_library_path()
    } else {
        body.path
    };
    let json = serde_json::to_string(&path).map_err(|_| StatusCode::BAD_REQUEST)?;
    state.device_config.save_app_state("library_path", &json);
    Ok(StatusCode::OK)
}

pub async fn set_volume(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    Json(body): Json<VolumeRequest>,
) -> Result<StatusCode, StatusCode> {
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let vol = (body.volume * 100.0).round() as u32;
    // Prefer HTTPS API — SOAP SetVolume on a master syncs to all slaves (firmware behavior)
    if let Some(ref https) = device.https_client {
        https
            .set_volume(vol)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
    } else {
        device
            .rendering
            .set_volume(vol)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
    }
    state.devices.update(&id, |d| d.volume = body.volume);
    state.events.publish(
        "volume_changed",
        &serde_json::json!({ "device_id": id, "volume": body.volume }),
    );
    Ok(StatusCode::OK)
}

pub async fn toggle_mute(
    State(state): State<ControlState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let new_mute = !device.muted;
    if let Some(ref https) = device.https_client {
        https
            .set_mute(new_mute)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
    } else {
        device
            .rendering
            .set_mute(new_mute)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
    }
    state.devices.update(&id, |d| d.muted = new_mute);
    state.events.publish(
        "mute_changed",
        &serde_json::json!({ "device_id": id, "muted": new_mute }),
    );
    Ok(StatusCode::OK)
}

pub async fn rename_device(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    Json(body): Json<DeviceNameRequest>,
) -> Result<StatusCode, StatusCode> {
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    device
        .rendering
        .set_device_name(&body.name)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    state.devices.update(&id, |d| d.name.clone_from(&body.name));
    state.events.publish(
        "device_state",
        &serde_json::json!({ "device_id": id, "name": body.name }),
    );
    Ok(StatusCode::OK)
}

pub async fn get_channel(
    State(state): State<ControlState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let channel = device
        .rendering
        .get_channel()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok(Json(serde_json::json!({ "channel": channel })))
}

pub async fn set_channel(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    Json(body): Json<ChannelRequest>,
) -> Result<StatusCode, StatusCode> {
    let device = state.devices.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    device
        .rendering
        .set_channel(&body.channel)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    state
        .devices
        .update(&id, |d| d.channel = Some(body.channel.clone()));
    state.events.publish(
        "device_state",
        &serde_json::json!({ "device_id": id, "channel": body.channel }),
    );
    Ok(StatusCode::OK)
}
