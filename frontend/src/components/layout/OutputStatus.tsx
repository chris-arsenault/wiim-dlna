import { useDeviceStore } from "../../stores/deviceStore";
import { usePlayerStore } from "../../stores/playerStore";

export function OutputStatus() {
  const devices = useDeviceStore((state) => state.devices);
  const enabled = devices.filter((device) => device.enabled && device.device_type === "wiim");
  const playing = usePlayerStore((state) => state.playing);
  const label = outputLabel(enabled.map((device) => device.name));

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-elevated)] border border-white/10 text-sm">
      <div
        className={`w-2 h-2 rounded-full ${playing && enabled.length > 0 ? "bg-emerald-400" : "bg-[var(--color-text-secondary)]"}`}
      />
      <span className="truncate max-w-[160px]">{label}</span>
    </div>
  );
}

function outputLabel(names: string[]): string {
  if (names.length === 0) return "No speakers on";
  if (names.length <= 2) return names.join(" + ");
  return `${names.length} speakers`;
}
