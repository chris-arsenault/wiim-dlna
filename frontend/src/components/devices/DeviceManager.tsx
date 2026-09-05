import { useState } from "react";
import { api, type Device } from "../../api/client";
import { useDeviceStore } from "../../stores/deviceStore";

export function DeviceManager() {
  const devices = useDeviceStore((state) => state.devices);
  const transitionActive = devices.some((device) => device.output_target != null);

  if (devices.length === 0) {
    return (
      <div className="space-y-3">
        <h2 className="text-xl font-semibold">Speakers</h2>
        <div className="text-center py-12">
          <div className="text-4xl mb-3">📡</div>
          <div className="text-sm text-[var(--color-text-secondary)]">
            Discovering WiiM speakers...
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-xl font-semibold">Speakers</h2>
        <p className="text-xs text-[var(--color-text-secondary)] mt-1">
          On speakers play the shared Airwave stream. Off speakers are detached and stopped.
        </p>
      </div>
      <div className="space-y-2">
        {devices.map((device) => (
          <OutputToggle key={device.id} device={device} transitionActive={transitionActive} />
        ))}
      </div>
    </div>
  );
}

function OutputToggle({ device, transitionActive }: { device: Device; transitionActive: boolean }) {
  const updateDevice = useDeviceStore((state) => state.updateDevice);
  const [requestPending, setRequestPending] = useState(false);
  const displayedEnabled = device.output_target ?? device.enabled;
  const status = outputStatus(device);

  const handleToggle = async () => {
    if (requestPending || transitionActive) return;
    const enabled = !device.enabled;
    setRequestPending(true);
    try {
      await api.setEnabled(device.id, enabled);
      updateDevice(device.id, { output_target: enabled, output_error: null });
    } catch {
      updateDevice(device.id, {
        output_error: `Could not start turning ${device.name} ${enabled ? "on" : "off"}`,
      });
    } finally {
      setRequestPending(false);
    }
  };

  return (
    <div className="bg-[var(--color-surface-elevated)] rounded-xl px-4 py-3">
      <div className="flex items-center gap-3">
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">{device.name}</div>
          <div className="text-xs text-[var(--color-text-secondary)] truncate mt-0.5">
            {device.model ?? device.ip}
          </div>
        </div>
        <span className={`text-xs ${outputStatusColor(displayedEnabled)}`}>{status}</span>
        <button
          type="button"
          role="switch"
          aria-checked={displayedEnabled}
          aria-label={`${device.name} output`}
          disabled={requestPending || transitionActive}
          onClick={handleToggle}
          className={`w-11 h-6 rounded-full transition-colors relative disabled:opacity-50 ${outputTrackColor(displayedEnabled)}`}
        >
          <span
            className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-all ${outputThumbPosition(displayedEnabled)}`}
          />
        </button>
      </div>
      {device.output_error && (
        <div className="text-xs text-red-400 mt-2">{device.output_error}</div>
      )}
    </div>
  );
}

function outputStatus(device: Device): string {
  if (device.output_target === true) return "Turning on…";
  if (device.output_target === false) return "Turning off…";
  return device.enabled ? "On" : "Off";
}

function outputStatusColor(enabled: boolean): string {
  return enabled ? "text-emerald-400" : "text-[var(--color-text-secondary)]";
}

function outputTrackColor(enabled: boolean): string {
  return enabled ? "bg-[var(--color-accent)]" : "bg-white/15";
}

function outputThumbPosition(enabled: boolean): string {
  return enabled ? "left-[22px]" : "left-[4px]";
}
