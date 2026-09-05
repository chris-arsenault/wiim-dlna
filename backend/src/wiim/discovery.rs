use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tracing::{debug, info, warn};

use super::collector::{CollectorClient, CollectorDevice};
use super::device::{DeviceCapabilities, DeviceManager, DeviceParams, ServiceUrls, WiimDevice};
use crate::control::events::EventBus;
use crate::control::state::ControlState;

fn proxy_service_urls(device: &CollectorDevice) -> ServiceUrls {
    let prefix = format!("/wiim/{}/upnp", device.id);
    ServiceUrls {
        av_transport: device
            .services
            .av_transport
            .as_ref()
            .map(|_| format!("{prefix}/av-transport")),
        rendering_control: device
            .services
            .rendering_control
            .as_ref()
            .map(|_| format!("{prefix}/rendering-control")),
        play_queue: device
            .services
            .play_queue
            .as_ref()
            .map(|_| format!("{prefix}/play-queue")),
    }
}

async fn probe_av_transport(device: &WiimDevice) -> bool {
    device.av_transport.get_transport_info().await.is_ok()
}

async fn probe_rendering_control(device: &WiimDevice) -> bool {
    device.rendering.get_volume().await.is_ok()
}

fn derive_group_role(
    slave_list: &str,
    status_json: &std::collections::HashMap<String, String>,
) -> (bool, bool) {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(slave_list) {
        if parsed
            .get("slaves")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|count| count > 0)
        {
            return (false, true);
        }
    }

    if status_json.get("group").is_some_and(|group| group != "0") {
        return (true, false);
    }
    (false, false)
}

fn resolve_master_id(
    status_json: &std::collections::HashMap<String, String>,
    device_manager: &DeviceManager,
) -> Option<String> {
    status_json
        .get("master_ip")
        .and_then(|ip| device_manager.find_id_by_ip(ip))
}

async fn register_device(
    record: CollectorDevice,
    forced_master_id: Option<String>,
    collector: &CollectorClient,
    device_manager: &DeviceManager,
    persisted: &std::collections::HashMap<String, crate::control::device_config::DeviceConfig>,
    events: &EventBus,
) -> Option<String> {
    if !record.reachable || !is_wiim_record(&record) {
        return None;
    }
    let id = record.id.clone();
    if device_manager.contains(&id) {
        if let Some(master_id) = forced_master_id {
            device_manager.update(&id, |device| {
                device.group_id = Some(master_id);
                device.is_master = false;
            });
        } else {
            refresh_group_state(&id, device_manager, events).await;
        }
        return Some(id);
    }

    let service_urls = proxy_service_urls(&record);
    let initial_caps = DeviceCapabilities {
        av_transport: service_urls.av_transport.is_some(),
        rendering_control: service_urls.rendering_control.is_some(),
        wiim_extended: service_urls.play_queue.is_some(),
        https_api: service_urls.play_queue.is_some(),
    };
    let mut device = WiimDevice::new(DeviceParams {
        ip: record.ip,
        port: record.description_port,
        name: record.name,
        model: record.model,
        firmware: record.firmware,
        udn: record.udn,
        service_urls,
        capabilities: initial_caps,
        collector_url: collector.base_url().to_string(),
        collector_token: collector.token().to_string(),
    });

    let av_ok = device.capabilities.av_transport && probe_av_transport(&device).await;
    let rc_ok = device.capabilities.rendering_control && probe_rendering_control(&device).await;
    if !av_ok && !rc_ok {
        debug!(
            "Device {} ({id}) did not respond through collector SOAP transport",
            device.name
        );
        return None;
    }
    device.capabilities.av_transport = av_ok;
    device.capabilities.rendering_control = rc_ok;

    if rc_ok {
        if let Ok(volume) = device.rendering.get_volume().await {
            device.volume = f64::from(volume) / 100.0;
        }
        if let Ok(muted) = device.rendering.get_mute().await {
            device.muted = muted;
        }
    }

    if let Some(https) = &device.https_client {
        let has_https = https.probe().await;
        device.capabilities.https_api = has_https;
        if !has_https {
            device.https_client = None;
            warn!(
                "LinkPlay API unavailable through collector for {} ({id}); EQ disabled",
                device.name
            );
        }
    } else {
        device.capabilities.https_api = false;
    }

    if device.capabilities.wiim_extended {
        if let Ok(device_info) = device.rendering.get_control_device_info().await {
            device.volume = f64::from(device_info.volume) / 100.0;
            device.muted = device_info.muted;
            device.channel = Some(device_info.channel.clone());
            device.name = device_info
                .raw
                .get("DeviceName")
                .or_else(|| device_info.raw.get("Name"))
                .cloned()
                .unwrap_or(device.name);

            let (is_slave, is_master) =
                derive_group_role(&device_info.slave_list, &device_info.raw);
            if is_master {
                device.is_master = true;
                device.group_id = Some(id.clone());
            } else if is_slave {
                device.group_id = resolve_master_id(&device_info.raw, device_manager);
            }
        }
    }
    if let Some(master_id) = forced_master_id {
        device.group_id = Some(master_id);
        device.is_master = false;
    }
    if let Some(config) = persisted.get(&id) {
        device.enabled = config.enabled;
    }

    info!(
        "Discovered {} device through collector: {} ({id}) at {}:{} [enabled={}, group={:?}, master={}]",
        device.device_type,
        device.name,
        device.ip,
        device.port,
        device.enabled,
        device.group_id,
        device.is_master,
    );
    events.publish(
        "device_added",
        &serde_json::json!({
            "id": device.id,
            "name": device.name,
            "ip": device.ip,
            "device_type": device.device_type,
        }),
    );
    device_manager.register(device);
    Some(id)
}

/// Poll the collector's native WiiM inventory and retain Airwave's device,
/// capability, group, and playback models on top of its scoped transport.
pub async fn run_discovery(state: ControlState, collector: CollectorClient, interval: Duration) {
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut known_ids = HashSet::new();

    loop {
        let persisted = state.device_config.load_all();
        let records = match collector.devices().await {
            Ok(records) => records,
            Err(error) => {
                state.collector_ready.store(false, Ordering::Relaxed);
                warn!("Failed to read WiiM inventory from collector: {error}");
                tokio::time::sleep(interval).await;
                continue;
            }
        };
        state.collector_ready.store(true, Ordering::Relaxed);
        let mut current_ids = HashSet::new();
        for record in records {
            if let Some(id) = register_device(
                record,
                None,
                &collector,
                &state.devices,
                &persisted,
                &state.events,
            )
            .await
            {
                current_ids.insert(id);
            }
        }

        // Grouped slaves may stop advertising. The master still reports their
        // addresses; ask the collector to validate and add each missing one.
        for master in state
            .devices
            .list_all()
            .iter()
            .filter(|device| device.is_master)
        {
            let Ok(info) = master.av_transport.get_info_ex().await else {
                continue;
            };
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&info.slave_list) else {
                continue;
            };
            let Some(slaves) = parsed
                .get("slave_list")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for slave in slaves {
                let Some(slave_ip) = slave.get("ip").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                if state
                    .devices
                    .find_id_by_ip(slave_ip)
                    .is_some_and(|id| current_ids.contains(&id))
                {
                    continue;
                }
                match collector.probe(slave_ip).await {
                    Ok(record) => {
                        if let Some(id) = register_device(
                            record,
                            Some(master.id.clone()),
                            &collector,
                            &state.devices,
                            &persisted,
                            &state.events,
                        )
                        .await
                        {
                            current_ids.insert(id);
                        }
                    }
                    Err(error) => {
                        debug!("Collector could not validate grouped slave at {slave_ip}: {error}")
                    }
                }
            }
        }

        for id in known_ids.difference(&current_ids) {
            info!("Device no longer reachable through collector: {id}");
            state.devices.remove(id);
            state
                .events
                .publish("device_removed", &serde_json::json!({ "id": id }));
        }
        known_ids = current_ids;
        if let Err(status) = crate::control::outputs::reconcile_outputs(&state).await {
            warn!("Failed to reconcile playing outputs: HTTP {status}");
        }
        tokio::time::sleep(interval).await;
    }
}

fn is_wiim_record(record: &CollectorDevice) -> bool {
    record.services.play_queue.is_some()
}

async fn refresh_group_state(device_id: &str, device_manager: &DeviceManager, events: &EventBus) {
    let Some(device) = device_manager.get(device_id) else {
        return;
    };
    if !device.capabilities.wiim_extended {
        return;
    }
    let Ok(device_info) = device.rendering.get_control_device_info().await else {
        return;
    };
    let (is_slave, is_master) = derive_group_role(&device_info.slave_list, &device_info.raw);
    let group_id = if is_master {
        Some(device_id.to_string())
    } else if is_slave {
        resolve_master_id(&device_info.raw, device_manager)
    } else {
        None
    };

    if device.group_id != group_id || device.is_master != is_master {
        info!(
            "Group state changed for {} ({device_id}): group={:?}->{:?}, master={}->{}",
            device.name, device.group_id, group_id, device.is_master, is_master
        );
        device_manager.update(device_id, |entry| {
            entry.group_id.clone_from(&group_id);
            entry.is_master = is_master;
        });
        publish_devices(device_manager, events);
    }

    let volume = f64::from(device_info.volume) / 100.0;
    if (device.volume - volume).abs() > 0.001 || device.muted != device_info.muted {
        device_manager.update(device_id, |entry| {
            entry.volume = volume;
            entry.muted = device_info.muted;
        });
    }
}

fn publish_devices(device_manager: &DeviceManager, events: &EventBus) {
    let devices = device_manager
        .list_all()
        .iter()
        .map(|device| {
            serde_json::json!({
                "id": device.id,
                "name": device.name,
                "ip": device.ip,
                "model": device.model,
                "firmware": device.firmware,
                "device_type": device.device_type,
                "enabled": device.enabled,
                "capabilities": device.capabilities,
                "volume": device.volume,
                "muted": device.muted,
                "channel": device.channel,
                "source": device.source,
                "group_id": device.group_id,
                "is_master": device.is_master,
            })
        })
        .collect::<Vec<_>>();
    events.publish(
        "devices_changed",
        &serde_json::json!({ "devices": devices }),
    );
}

#[cfg(test)]
mod tests {
    use super::super::collector::CollectorServices;
    use super::*;

    #[test]
    fn inventory_services_map_to_fixed_collector_routes() {
        let device = CollectorDevice {
            id: "wiim-office".into(),
            udn: "uuid:wiim-office".into(),
            ip: "192.168.30.20".into(),
            name: "Office".into(),
            model: None,
            firmware: None,
            description_port: 49152,
            services: CollectorServices {
                av_transport: Some("/device/native-av".into()),
                rendering_control: Some("/device/native-rc".into()),
                play_queue: None,
            },
            reachable: true,
        };
        let services = proxy_service_urls(&device);
        assert_eq!(
            services.av_transport.as_deref(),
            Some("/wiim/wiim-office/upnp/av-transport")
        );
        assert_eq!(
            services.rendering_control.as_deref(),
            Some("/wiim/wiim-office/upnp/rendering-control")
        );
        assert!(services.play_queue.is_none());
    }

    #[test]
    fn group_role_still_comes_from_device_state() {
        let status = std::collections::HashMap::from([("group".into(), "0".into())]);
        assert_eq!(derive_group_role(r#"{"slaves":2}"#, &status), (false, true));
        let grouped = std::collections::HashMap::from([("group".into(), "1".into())]);
        assert_eq!(derive_group_role("{}", &grouped), (true, false));
    }

    #[test]
    fn inventory_without_wiim_play_queue_is_filtered() {
        let mut device = CollectorDevice {
            id: "living-room-tv".into(),
            udn: "uuid:living-room-tv".into(),
            ip: "192.0.2.10".into(),
            name: "TV".into(),
            model: None,
            firmware: None,
            description_port: 1400,
            services: CollectorServices {
                av_transport: Some("/MediaRenderer/AVTransport/Control".into()),
                rendering_control: Some("/MediaRenderer/RenderingControl/Control".into()),
                play_queue: None,
            },
            reachable: true,
        };
        assert!(!is_wiim_record(&device));
        device.services.play_queue = Some("/upnp/control/PlayQueue1".into());
        assert!(is_wiim_record(&device));
    }
}
