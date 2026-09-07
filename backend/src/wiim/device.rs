use dashmap::DashMap;
use serde::Serialize;

use super::https_api::HttpsApiClient;
use super::services::av_transport::AvTransport;
use super::services::play_queue::PlayQueueService;
use super::services::rendering_control::RenderingControl;
use super::soap_client::SoapClient;

/// Capabilities detected from the device's description.xml and SOAP probes.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceCapabilities {
    pub av_transport: bool,
    pub rendering_control: bool,
    pub wiim_extended: bool,
    pub https_api: bool,
}

/// Service control URLs parsed from a device's description.xml.
#[derive(Debug, Clone, Default)]
pub struct ServiceUrls {
    pub av_transport: Option<String>,
    pub rendering_control: Option<String>,
    pub play_queue: Option<String>,
}

/// Parameters for constructing a new device.
pub struct DeviceParams {
    pub ip: String,
    pub port: u16,
    pub name: String,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub udn: String,
    pub service_urls: ServiceUrls,
    pub capabilities: DeviceCapabilities,
    pub collector_url: String,
    pub collector_token: String,
}

/// A discovered UPnP MediaRenderer device with its service clients.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WiimDevice {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub udn: String,
    pub device_type: String,
    /// Persisted desired participation in the one Airwave playing group.
    pub enabled: bool,
    /// Desired state currently being reconciled with WiiM hardware.
    pub output_target: Option<bool>,
    /// Last bounded output-transition failure. The global recovery latch owns
    /// whether hardware commands may run again.
    pub output_error: Option<String>,
    pub capabilities: DeviceCapabilities,
    /// User-controlled per-speaker base level before global scaling.
    pub volume: f64,
    /// Last physical volume observed or written after global scaling.
    pub applied_volume: f64,
    pub muted: bool,
    pub channel: Option<String>,
    pub source: Option<String>,
    pub group_id: Option<String>,
    pub is_master: bool,
    pub av_transport: AvTransport,
    pub rendering: RenderingControl,
    pub play_queue: PlayQueueService,
    pub https_client: Option<HttpsApiClient>,
}

impl WiimDevice {
    pub fn new(params: DeviceParams) -> Self {
        let id = params.udn.replace("uuid:", "");
        let client = SoapClient::new(params.collector_url.clone(), &params.collector_token);

        let device_type = if params.capabilities.wiim_extended {
            "wiim".to_string()
        } else {
            "renderer".to_string()
        };

        let av_transport = if let Some(url) = params.service_urls.av_transport {
            AvTransport::with_control_url(client.clone(), url)
        } else {
            AvTransport::new(client.clone())
        };

        let rendering = if let Some(url) = params.service_urls.rendering_control {
            RenderingControl::with_control_url(client.clone(), url)
        } else {
            RenderingControl::new(client.clone())
        };

        let play_queue = if let Some(url) = params.service_urls.play_queue {
            PlayQueueService::with_control_url(client, url)
        } else {
            PlayQueueService::new(client)
        };

        let https_client = if params.capabilities.https_api {
            Some(HttpsApiClient::new(
                format!("{}/wiim/{id}/linkplay", params.collector_url),
                &params.collector_token,
            ))
        } else {
            None
        };

        Self {
            id,
            name: params.name,
            ip: params.ip,
            port: params.port,
            model: params.model,
            firmware: params.firmware,
            udn: params.udn,
            device_type,
            enabled: true,
            output_target: None,
            output_error: None,
            capabilities: params.capabilities,
            volume: 0.0,
            applied_volume: 0.0,
            muted: false,
            channel: None,
            source: None,
            group_id: None,
            is_master: false,
            av_transport,
            rendering,
            play_queue,
            https_client,
        }
    }
}

/// Thread-safe registry of discovered devices.
pub struct DeviceManager {
    devices: DashMap<String, WiimDevice>,
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: DashMap::new(),
        }
    }

    pub fn register(&self, device: WiimDevice) {
        self.devices.insert(device.id.clone(), device);
    }

    pub fn get(&self, id: &str) -> Option<WiimDevice> {
        self.devices.get(id).map(|d| d.value().clone())
    }

    pub fn list_all(&self) -> Vec<WiimDevice> {
        self.devices.iter().map(|r| r.value().clone()).collect()
    }

    pub fn update<F: FnOnce(&mut WiimDevice)>(&self, id: &str, f: F) {
        if let Some(mut device) = self.devices.get_mut(id) {
            f(device.value_mut());
        }
    }

    pub fn remove(&self, id: &str) {
        self.devices.remove(id);
    }

    pub fn contains(&self, id: &str) -> bool {
        self.devices.contains_key(id)
    }

    pub fn find_id_by_ip(&self, ip: &str) -> Option<String> {
        self.devices
            .iter()
            .find(|r| r.value().ip == ip)
            .map(|r| r.key().clone())
    }
}
