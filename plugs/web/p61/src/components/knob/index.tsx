import useGesture from "./useGesture.ts";
import Display from "./Display.tsx";
import Label from "./Label.tsx";
import { useMemo } from "react";

export interface Props {
  /**
   * The current value of the knob (scaled to 0-100)
   */
  value: number;

  /**
   * True if the knob is grabbed
   */
  grabbed?: boolean;

  /**
   * Callback for when the knob is grabbed or release through a pointer event.
   * Note that this may be called spruriously even if the grabbed state didn't change.
   */
  onGrabOrRelease?: (grabbed: boolean) => void;

  /**
   * Callback for when the value of the knob changes.
   * Note that this may be called spuriously even if the value didn't change.
   */
  onValue?: (value: number) => void;

  /**
   * The label of the knob. Note this is required for accessibility. To hide the label, set `showLabel` to false.
   */
  label: string;

  /**
   * Whether we should show the label
   */
  showLabel?: boolean;

  /**
   * Value formatter to convert values into strings
   */
  valueFormatter?: (value: number) => string;

  /**
   * The style of the knob
   */
  style?: "primary" | "secondary";

  /**
   * Label for accessibility (can contain more information than `label`)
   */
  accessibilityLabel?: string;
}

const PRIMARY_KNOB_SIZE = 61;
const SECONDARY_KNOB_SIZE = 31;
const PRIMARY_RADIUS_RATIO = 18 / 30.5;
const SECONDARY_RADIUS_RATIO = 15 / 30.5;

const Knob = ({
  value,
  grabbed,
  onGrabOrRelease,
  onValue,
  label,
  valueFormatter,
  showLabel = true,
  style = "primary",
  accessibilityLabel,
}: Props) => {
  const { hover, props } = useGesture({ value, onGrabOrRelease, onValue });
  const valueLabel = useMemo(
    () => (valueFormatter ? valueFormatter(value) : label),
    [valueFormatter, value, label],
  );
  return (
    <div
      role="slider"
      aria-label={accessibilityLabel ?? label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value}
      aria-orientation="vertical"
      aria-valuetext={valueFormatter ? valueLabel : String(value)}
      tabIndex={0}
      className="inline-block cursor-default touch-none select-none"
      {...props}
    >
      <Display
        size={style === "primary" ? PRIMARY_KNOB_SIZE : SECONDARY_KNOB_SIZE}
        innerRadiusRatio={
          style === "primary" ? PRIMARY_RADIUS_RATIO : SECONDARY_RADIUS_RATIO
        }
        value={value}
        grabbed={grabbed ?? false}
        hover={hover}
      />
      {showLabel ? (
        // Note that `valueLabel` will never be undefined if `label` is defined.
        <Label
          label={label}
          hover={hover || !!grabbed}
          valueLabel={valueLabel}
        />
      ) : undefined}
    </div>
  );
};

export default Knob;
