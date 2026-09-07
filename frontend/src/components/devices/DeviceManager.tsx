import { useCallback, useState } from "react";
import { api, type Device } from "../../api/client";
import { setDeviceVolume } from "../../hooks/useDeviceVolume";
import { useDeviceStore } from "../../stores/deviceStore";
import { VolumeSlider } from "../VolumeSlider";

export function DeviceManager() {
  const devices = useDeviceStore((state) => state.devices);
  const recovery = useDeviceStore((state) => state.outputRecovery);
  const setOutputRecovery = useDeviceStore((state) => state.setOutputRecovery);
  const [recoveryRequestPending, setRecoveryRequestPending] = useState(false);
  const transitionActive =
    recovery.in_progress || devices.some((device) => device.output_target != null);

  const handleRecover = useCallback(async () => {
    if (recoveryRequestPending || transitionActive) return;
    setRecoveryRequestPending(true);
    try {
      await api.recoverOutputs();
      setOutputRecovery({ ...recovery, in_progress: true, error: null });
    } catch {
      setOutputRecovery({
        ...recovery,
        error: "Could not start speaker recovery",
      });
    } finally {
      setRecoveryRequestPending(false);
    }
  }, [recovery, recoveryRequestPending, setOutputRecovery, transitionActive]);

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
      <RecoveryControl
        required={recovery.required}
        inProgress={recovery.in_progress}
        error={recovery.error}
        requestPending={recoveryRequestPending}
        onRecover={handleRecover}
      />
      <div className="space-y-2">
        {devices.map((device) => (
          <OutputToggle
            key={device.id}
            device={device}
            transitionActive={transitionActive}
            recoveryRequired={recovery.required}
          />
        ))}
      </div>
    </div>
  );
}

function RecoveryControl({
  required,
  inProgress,
  error,
  requestPending,
  onRecover,
}: {
  required: boolean;
  inProgress: boolean;
  error: string | null;
  requestPending: boolean;
  onRecover: () => void;
}) {
  let title = "Speaker group recovery";
  if (inProgress) title = "Recovering speaker group…";
  else if (required) title = "Speaker group needs recovery";

  return (
    <div
      className={`rounded-xl px-4 py-3 ${required ? "bg-red-500/10 border border-red-500/30" : "bg-[var(--color-surface-elevated)]"}`}
    >
      <div className="flex items-center gap-3">
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium">{title}</div>
          <div className="text-xs text-[var(--color-text-secondary)] mt-1">
            {required
              ? "Toggle changes are saved, but no speaker commands run until recovery is requested."
              : "Rebuild every speaker from a known standalone state if the physical group is stuck."}
          </div>
        </div>
        <button
          type="button"
          disabled={requestPending || inProgress}
          onClick={onRecover}
          className="rounded-lg px-3 py-2 text-xs font-medium bg-white/10 hover:bg-white/15 disabled:opacity-50"
        >
          {inProgress ? "Recovering…" : "Recover speakers"}
        </button>
      </div>
      {error && <div className="text-xs text-red-400 mt-2">{error}</div>}
    </div>
  );
}

function OutputToggle({
  device,
  transitionActive,
  recoveryRequired,
}: {
  device: Device;
  transitionActive: boolean;
  recoveryRequired: boolean;
}) {
  const updateDevice = useDeviceStore((state) => state.updateDevice);
  const [requestPending, setRequestPending] = useState(false);
  const displayedEnabled = device.enabled;
  const status = outputStatus(device, recoveryRequired);
  const handleVolumeCommit = useCallback(
    (volume: number) => setDeviceVolume(device.id, volume),
    [device.id]
  );

  const handleToggle = async () => {
    if (requestPending || transitionActive) return;
    const enabled = !device.enabled;
    setRequestPending(true);
    try {
      await api.setEnabled(device.id, enabled);
      updateDevice(device.id, {
        enabled,
        output_target: recoveryRequired ? null : enabled,
        output_error: null,
      });
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
      <div className="mt-3">
        <VolumeSlider
          value={device.volume}
          label="Level"
          ariaLabel={`${device.name} volume`}
          disabled={requestPending || transitionActive}
          onCommit={handleVolumeCommit}
        />
      </div>
      {device.output_error && !recoveryRequired && (
        <div className="text-xs text-red-400 mt-2">{device.output_error}</div>
      )}
    </div>
  );
}

function outputStatus(device: Device, recoveryRequired: boolean): string {
  if (device.output_target === true) return "Turning on…";
  if (device.output_target === false) return "Turning off…";
  if (recoveryRequired) return device.enabled ? "Wanted on" : "Wanted off";
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
