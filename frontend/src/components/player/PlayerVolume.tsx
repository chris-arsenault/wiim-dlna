import { VolumeSlider } from "../VolumeSlider";
import { usePlaybackVolumeActions } from "../../hooks/useDeviceVolume";
import { selectPlaybackDevice, useDeviceStore } from "../../stores/deviceStore";
import { usePlayerStore } from "../../stores/playerStore";

export function PlayerVolume() {
  const activeDevice = useDeviceStore((state) => selectPlaybackDevice(state.devices));
  const transitionActive = useDeviceStore((state) =>
    state.devices.some((device) => device.output_target != null)
  );
  const volume = usePlayerStore((state) => state.volume);
  const { setVolume } = usePlaybackVolumeActions();

  if (!activeDevice?.enabled || !activeDevice.capabilities.rendering_control) return null;

  return (
    <div className="px-8 py-1 shrink-0">
      <VolumeSlider
        value={volume}
        label="Main"
        ariaLabel="Main volume"
        disabled={transitionActive}
        onCommit={setVolume}
      />
    </div>
  );
}
