import { useAtom } from "jotai";
import { clockRateAtom } from "./Controls";

export const ClockRateControl = () => {
  const [clockRate, setClockRate] = useAtom(clockRateAtom);

  return (
    <div className="flex flex-col gap-2">
      <label className="flex items-center gap-2">
        <span>Clock Rate: {clockRate.toFixed(1)} Hz</span>
        <input
          type="range"
          min={0.25}
          max={4}
          step={0.25}
          value={clockRate}
          onChange={(e) => {
            setClockRate(parseFloat(e.target.value));
          }}
          style={{ marginLeft: "1rem" }}
        />
      </label>
    </div>
  );
};

export default ClockRateControl;
