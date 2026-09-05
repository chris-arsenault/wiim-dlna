import { useState } from "react";
import { api, type Device } from "../../api/client";
import { useDeviceStore } from "../../stores/deviceStore";

export function DeviceManager() {
  const devices = useDeviceStore((state) => state.devices);

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
          <OutputToggle key={device.id} device={device} />
        ))}
      </div>
    </div>
  );
}

function OutputToggle({ device }: { device: Device }) {
  const updateDevice = useDeviceStore((state) => state.updateDevice);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleToggle = async () => {
    if (pending) return;
    const enabled = !device.enabled;
    setPending(true);
    setError(null);
    try {
      await api.setEnabled(device.id, enabled);
      updateDevice(device.id, { enabled });
    } catch {
      setError(`Could not turn ${device.name} ${enabled ? "on" : "off"}`);
    } finally {
      setPending(false);
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
        <span
          className={`text-xs ${device.enabled ? "text-emerald-400" : "text-[var(--color-text-secondary)]"}`}
        >
          {device.enabled ? "On" : "Off"}
        </span>
        <button
          type="button"
          role="switch"
          aria-checked={device.enabled}
          aria-label={`${device.name} output`}
          disabled={pending}
          onClick={handleToggle}
          className={`w-11 h-6 rounded-full transition-colors relative disabled:opacity-50 ${
            device.enabled ? "bg-[var(--color-accent)]" : "bg-white/15"
          }`}
        >
          <span
            className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-all ${
              device.enabled ? "left-[22px]" : "left-[4px]"
            }`}
          />
        </button>
      </div>
      {error && <div className="text-xs text-red-400 mt-2">{error}</div>}
    </div>
  );
}
