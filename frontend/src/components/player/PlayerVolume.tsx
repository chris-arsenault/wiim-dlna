import { usePlaybackVolumeActions, volumePercent } from "../../hooks/useDeviceVolume";
import { selectPlaybackDevice, useDeviceStore } from "../../stores/deviceStore";

export function PlayerVolume() {
  const activeDevice = useDeviceStore((state) => selectPlaybackDevice(state.devices));
  const { setVolume } = usePlaybackVolumeActions();

  if (!activeDevice?.enabled || !activeDevice.capabilities.rendering_control) return null;

  const percent = volumePercent(activeDevice);

  const handleVolumeChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    await setVolume(Number(e.target.value) / 100);
  };

  return (
    <div className="px-8 py-1 shrink-0">
      <div className="flex items-center gap-3">
        <span className="text-xs text-white/50 w-8">Vol</span>
        <input
          type="range"
          min={0}
          max={100}
          value={percent}
          onChange={handleVolumeChange}
          aria-label="Playing group volume"
          className="seek-bar flex-1"
        />
        <span className="text-xs tabular-nums text-white/60 w-8 text-right">{percent}</span>
      </div>
    </div>
  );
}
