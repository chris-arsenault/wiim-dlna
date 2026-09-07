use thiserror::Error;

use crate::wiim::device::WiimDevice;

use super::device_config::DeviceConfigStore;

pub const GLOBAL_VOLUME_KEY: &str = "global_volume_multiplier";

#[derive(Debug, Error)]
pub enum VolumeWriteError {
    #[error("speaker {0} has no direct Linkplay volume control")]
    DirectControlUnavailable(String),
    #[error("could not set volume on {device}: {message}")]
    Write { device: String, message: String },
}

pub fn load_global_volume(store: &DeviceConfigStore) -> f64 {
    store
        .load_app_state(GLOBAL_VOLUME_KEY)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| (0.0..=1.0).contains(value))
        .unwrap_or(1.0)
}

pub fn save_global_volume(store: &DeviceConfigStore, volume: f64) {
    store.save_app_state(GLOBAL_VOLUME_KEY, &volume.to_string());
}

pub fn effective_volume(base_volume: f64, global_volume: f64) -> f64 {
    (base_volume.clamp(0.0, 1.0) * global_volume.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

pub fn reconcile_observed_volume(
    device: &mut WiimDevice,
    observed_volume: f64,
    global_volume: f64,
    infer_base_volume: bool,
) -> Option<f64> {
    let observed_volume = observed_volume.clamp(0.0, 1.0);
    if (device.applied_volume - observed_volume).abs() <= 0.005 {
        return None;
    }
    device.applied_volume = observed_volume;

    let expected = effective_volume(device.volume, global_volume);
    if !infer_base_volume || global_volume <= 0.005 || (expected - observed_volume).abs() <= 0.015 {
        return None;
    }

    let base_volume = (observed_volume / global_volume).clamp(0.0, 1.0);
    device.volume = base_volume;
    Some(base_volume)
}

pub async fn write_effective_volume(
    device: &WiimDevice,
    effective_volume: f64,
) -> Result<(), VolumeWriteError> {
    let https = device
        .https_client
        .as_ref()
        .ok_or_else(|| VolumeWriteError::DirectControlUnavailable(device.id.clone()))?;
    let percent = (effective_volume.clamp(0.0, 1.0) * 100.0).round() as u32;
    https
        .set_volume(percent)
        .await
        .map_err(|error| VolumeWriteError::Write {
            device: device.id.clone(),
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_volume_scales_without_flattening_device_balance() {
        assert!((effective_volume(0.8, 0.5) - 0.4).abs() < f64::EPSILON);
        assert!((effective_volume(0.3, 0.5) - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn effective_volume_clamps_both_inputs() {
        assert_eq!(effective_volume(2.0, 2.0), 1.0);
        assert_eq!(effective_volume(-1.0, 0.5), 0.0);
    }

    #[test]
    fn global_volume_is_persisted_and_defaults_safely() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("device_config.db");
        let store = DeviceConfigStore::new(path.to_str().unwrap());

        assert_eq!(load_global_volume(&store), 1.0);
        save_global_volume(&store, 0.42);
        assert_eq!(load_global_volume(&store), 0.42);

        store.save_app_state(GLOBAL_VOLUME_KEY, "not-a-volume");
        assert_eq!(load_global_volume(&store), 1.0);
    }

    fn test_device() -> WiimDevice {
        let mut device = WiimDevice::new(crate::wiim::device::DeviceParams {
            ip: "192.168.1.10".to_string(),
            port: 49152,
            name: "Kitchen".to_string(),
            model: None,
            firmware: None,
            udn: "uuid:kitchen".to_string(),
            service_urls: crate::wiim::device::ServiceUrls::default(),
            capabilities: crate::wiim::device::DeviceCapabilities {
                av_transport: false,
                rendering_control: false,
                wiim_extended: false,
                https_api: false,
            },
            collector_url: "http://collector".to_string(),
            collector_token: "test".to_string(),
        });
        device.volume = 0.8;
        device.applied_volume = 0.4;
        device
    }

    #[test]
    fn transition_observation_does_not_replace_the_speaker_base_level() {
        let mut device = test_device();

        assert_eq!(
            reconcile_observed_volume(&mut device, 0.3, 0.5, false),
            None
        );
        assert_eq!(device.volume, 0.8);
        assert_eq!(device.applied_volume, 0.3);
    }

    #[test]
    fn stable_physical_change_updates_the_base_level() {
        let mut device = test_device();

        assert_eq!(
            reconcile_observed_volume(&mut device, 0.3, 0.5, true),
            Some(0.6)
        );
        assert_eq!(device.volume, 0.6);
        assert_eq!(device.applied_volume, 0.3);
    }
}
