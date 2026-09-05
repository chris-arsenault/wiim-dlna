import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Device, type EqBand } from "../../api/client";
import { useDeviceStore } from "../../stores/deviceStore";
import { AudioTab } from "./AudioTab";

const BAND_LABELS = ["31", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];
const BUILTIN_PRESETS = new Set([
  "Flat",
  "Acoustic",
  "Bass Booster",
  "Bass Reducer",
  "Classical",
  "Dance",
  "Deep",
  "Electronic",
  "Game",
  "Hip-Hop",
  "Jazz",
  "Latin",
  "Loudness",
  "Lounge",
  "Movie",
  "Piano",
  "Pop",
  "R&B",
  "Rock",
  "Small Speakers",
  "Spoken Word",
  "Treble Booster",
  "Treble Reducer",
  "Vocal Booster",
]);

const TAB_LABELS: Record<string, string> = {
  presets: "Presets",
  bands: "EQ Bands",
  audio: "Audio",
};

function tabLabel(tab: string): string {
  return TAB_LABELS[tab] ?? tab;
}

export function EQSettings() {
  const devices = useDeviceStore((state) => state.devices);
  const settingsDeviceId = useDeviceStore((state) => state.settingsDeviceId);
  const setSettingsDevice = useDeviceStore((state) => state.setSettingsDevice);
  const activeDevice = devices.find((device) => device.id === settingsDeviceId);
  const [activeTab, setActiveTab] = useState<"presets" | "bands" | "audio">("presets");

  if (!settingsDeviceId || !activeDevice) {
    return (
      <div className="space-y-3">
        <h2 className="text-xl font-semibold">Settings</h2>
        <div className="text-center py-12 text-[var(--color-text-secondary)] text-sm">
          No WiiM speakers found
        </div>
      </div>
    );
  }

  if (!activeDevice.capabilities.https_api) {
    return (
      <div className="space-y-3">
        <h2 className="text-xl font-semibold">Settings</h2>
        <DeviceHeader device={activeDevice} devices={devices} onSelect={setSettingsDevice} />
        <div className="text-center py-12">
          <div className="text-amber-400 text-sm font-medium mb-2">EQ Unavailable</div>
          <div className="text-xs text-[var(--color-text-secondary)]">
            HTTPS API (port 443) is not responding on this device.
            <br />
            Try rebooting the device to restore EQ controls.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-xl font-semibold">Settings</h2>
      <DeviceHeader device={activeDevice} devices={devices} onSelect={setSettingsDevice} />

      <div className="flex gap-1 bg-[var(--color-surface-elevated)] rounded-lg p-1">
        {(["presets", "bands", "audio"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`flex-1 text-sm py-2 rounded-md transition-colors ${
              activeTab === tab
                ? "bg-[var(--color-accent)] text-white"
                : "text-[var(--color-text-secondary)]"
            }`}
          >
            {tabLabel(tab)}
          </button>
        ))}
      </div>

      {activeTab === "presets" && <PresetsTab deviceId={settingsDeviceId} />}
      {activeTab === "bands" && <BandsTab deviceId={settingsDeviceId} />}
      {activeTab === "audio" && <AudioTab deviceId={settingsDeviceId} />}
    </div>
  );
}

function DeviceHeader({
  device,
  devices,
  onSelect,
}: {
  device: Device;
  devices: Device[];
  onSelect: (id: string) => void;
}) {
  return (
    <div className="bg-[var(--color-surface-elevated)] rounded-xl p-4">
      <label className="block text-xs text-[var(--color-text-secondary)] mb-1" htmlFor="eq-device">
        Speaker settings
      </label>
      <select
        id="eq-device"
        value={device.id}
        onChange={(event) => onSelect(event.target.value)}
        className="w-full bg-white/5 rounded-lg px-2 py-1.5 text-sm font-medium outline-none border border-white/10"
      >
        {devices.map((option) => (
          <option key={option.id} value={option.id}>
            {option.name}
          </option>
        ))}
      </select>
      <div className="text-xs text-[var(--color-text-secondary)] mt-0.5">
        {device.ip}
        {device.model ? ` · ${device.model}` : ""}
        {device.firmware ? ` · FW ${device.firmware}` : ""}
      </div>
    </div>
  );
}

function EqToggle({
  enabled,
  currentPreset,
  onToggle,
}: {
  enabled: boolean;
  currentPreset: string;
  onToggle: () => void;
}) {
  return (
    <div className="flex items-center justify-between bg-[var(--color-surface-elevated)] rounded-xl px-4 py-3">
      <div>
        <div className="text-sm font-medium">Equalizer</div>
        {enabled && currentPreset && (
          <div className="text-xs text-[var(--color-text-secondary)] mt-0.5">{currentPreset}</div>
        )}
      </div>
      <ToggleSwitch enabled={enabled} onToggle={onToggle} />
    </div>
  );
}

function ToggleSwitch({ enabled, onToggle }: { enabled: boolean; onToggle: () => void }) {
  return (
    <button
      onClick={onToggle}
      className={`w-11 h-6 rounded-full transition-colors relative ${
        enabled ? "bg-[var(--color-accent)]" : "bg-white/15"
      }`}
    >
      <div
        className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-all ${
          enabled ? "left-[22px]" : "left-[4px]"
        }`}
      />
    </button>
  );
}

function PresetsTab({ deviceId }: { deviceId: string }) {
  const queryClient = useQueryClient();

  const stateQuery = useQuery({
    queryKey: ["eq", "state", deviceId],
    queryFn: () => api.getEqState(deviceId),
  });

  const presetsQuery = useQuery({
    queryKey: ["eq", "presets", deviceId],
    queryFn: () => api.getEqPresets(deviceId),
  });

  const handleToggleEq = async () => {
    if (!stateQuery.data) return;
    if (stateQuery.data.enabled) await api.disableEq(deviceId);
    else await api.enableEq(deviceId);
    queryClient.invalidateQueries({ queryKey: ["eq", "state", deviceId] });
  };

  const handleSelectPreset = async (preset: string) => {
    await api.loadEqPreset(deviceId, preset);
    queryClient.invalidateQueries({ queryKey: ["eq", "state", deviceId] });
  };

  const handleDeletePreset = async (name: string) => {
    await api.deleteEqPreset(deviceId, name);
    queryClient.invalidateQueries({ queryKey: ["eq", "presets", deviceId] });
  };

  const presets = presetsQuery.data?.presets ?? [];
  const currentPreset = stateQuery.data?.preset_name ?? "";
  const eqEnabled = stateQuery.data?.enabled ?? false;

  return (
    <div className="space-y-3">
      <EqToggle enabled={eqEnabled} currentPreset={currentPreset} onToggle={handleToggleEq} />
      <PresetGrid
        presets={presets}
        currentPreset={currentPreset}
        eqEnabled={eqEnabled}
        onSelect={handleSelectPreset}
        onDelete={handleDeletePreset}
      />
    </div>
  );
}

function PresetGrid({
  presets,
  currentPreset,
  eqEnabled,
  onSelect,
  onDelete,
}: {
  presets: string[];
  currentPreset: string;
  eqEnabled: boolean;
  onSelect: (preset: string) => void;
  onDelete: (preset: string) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-2">
      {presets.map((preset) => (
        <div key={preset} className="relative group">
          <button
            onClick={() => onSelect(preset)}
            className={`w-full rounded-lg px-3 py-3 text-sm text-left transition-colors active:scale-[0.98] ${
              eqEnabled && currentPreset === preset
                ? "bg-[var(--color-accent)]/20 text-[var(--color-accent)] ring-1 ring-[var(--color-accent)]/40"
                : "bg-[var(--color-surface-elevated)] hover:bg-[var(--color-surface-hover)]"
            }`}
          >
            {preset}
          </button>
          {!BUILTIN_PRESETS.has(preset) && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onDelete(preset);
              }}
              className="absolute top-1 right-1 w-5 h-5 rounded-full bg-red-500/20 text-red-400 text-xs flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
              title="Delete preset"
            >
              &times;
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

function BandsTab({ deviceId }: { deviceId: string }) {
  const queryClient = useQueryClient();
  const [saveName, setSaveName] = useState("");
  const [saving, setSaving] = useState(false);

  const stateQuery = useQuery({
    queryKey: ["eq", "state", deviceId],
    queryFn: () => api.getEqState(deviceId),
  });

  const bands = stateQuery.data?.bands ?? [];
  const eqEnabled = stateQuery.data?.enabled ?? false;

  const handleBandChange = useCallback(
    async (band: EqBand, rawValue: number) => {
      // rawValue is 0-100 from the slider, convert to dB: (rawValue - 50) * 0.24 gives roughly -12 to +12
      await api.setEqBand(deviceId, band.index, rawValue);
      queryClient.invalidateQueries({ queryKey: ["eq", "state", deviceId] });
    },
    [deviceId, queryClient]
  );

  const handleReset = async () => {
    await api.loadEqPreset(deviceId, "Flat");
    queryClient.invalidateQueries({ queryKey: ["eq", "state", deviceId] });
  };

  const handleSave = async () => {
    if (!saveName.trim()) return;
    setSaving(true);
    await api.saveEqPreset(deviceId, saveName.trim());
    queryClient.invalidateQueries({ queryKey: ["eq", "presets", deviceId] });
    setSaveName("");
    setSaving(false);
  };

  if (!eqEnabled) {
    return (
      <div className="text-center py-12 text-[var(--color-text-secondary)] text-sm">
        Enable the equalizer on the Presets tab to adjust bands
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Band sliders */}
      <div className="bg-[var(--color-surface-elevated)] rounded-xl p-4 space-y-3">
        {bands.map((band, i) => (
          <BandSlider
            key={band.index}
            band={band}
            label={BAND_LABELS[i] ?? band.param_name}
            onChange={handleBandChange}
          />
        ))}
      </div>

      {/* Reset + Save */}
      <div className="flex gap-2">
        <button
          onClick={handleReset}
          className="text-xs text-[var(--color-text-secondary)] px-3 py-2 rounded-lg border border-white/10 hover:bg-white/5 transition-colors"
        >
          Reset to Flat
        </button>
        <input
          type="text"
          value={saveName}
          onChange={(e) => setSaveName(e.target.value)}
          placeholder="Preset name..."
          className="flex-1 text-xs bg-[var(--color-surface-elevated)] rounded-lg px-3 py-2 outline-none"
        />
        <button
          onClick={handleSave}
          disabled={!saveName.trim() || saving}
          className="text-xs text-[var(--color-accent)] px-3 py-2 rounded-lg border border-[var(--color-accent)]/30 hover:bg-[var(--color-accent)]/10 transition-colors disabled:opacity-40"
        >
          Save
        </button>
      </div>
    </div>
  );
}

function BandSlider({
  band,
  label,
  onChange,
}: {
  band: EqBand;
  label: string;
  onChange: (band: EqBand, value: number) => void;
}) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => onChange(band, value), 200);
  };

  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-[var(--color-text-secondary)] w-8 text-right shrink-0">
        {label}
      </span>
      <input
        type="range"
        min={0}
        max={100}
        defaultValue={Math.round(band.value)}
        onChange={handleChange}
        className="flex-1 h-1 accent-[var(--color-accent)] bg-white/10 rounded-full appearance-none cursor-pointer"
      />
      <span className="text-xs text-[var(--color-text-secondary)] w-6 text-right shrink-0">
        {Math.round(band.value)}
      </span>
    </div>
  );
}
