import { useCallback, useEffect, useRef } from "react";
import { indexOf } from "../util";
import { LabelGroup, ValueLabel } from "./value-label";
export type { ValueLabel, ValueLabelProps } from "./value-label";
export interface SliderProps {
  index: number | undefined;
  count: number;
  selectIndex: (index: number) => void;
  onGrabOrRelease?: (grabbed: boolean) => void;
  grabbed: boolean;
}

export type Slider = React.FC<SliderProps>;

export interface Props {
  /**
   * The possible values of the enum
   */
  values: string[];

  /**
   * True if the slider is grabbed
   */
  grabbed?: boolean;

  /**
   * The current value of the enum - must be one of `values`
   */
  value: string;

  /**
   * Accessibility label for the enum - can contain more information than `label`
   */
  accessibilityLabel: string;

  /**
   * Callback for when the value of the enum changes.
   */
  onValue?: (value: string) => void;

  /**
   * Callback for when the slider is grabbed or release through a pointer event.
   * Note that this may be called spruriously even if the grabbed state didn't change.
   */
  onGrabOrRelease?: (grabbed: boolean) => void;

  /**
   * Display formatter, if applicable. By default just shows the value.
   */
  displayFormatter?: (value: string) => string;

  ValueLabel: ValueLabel;
  Slider: Slider;
}

export const EnumSlider = ({
  value,
  values,
  onValue,
  onGrabOrRelease,
  accessibilityLabel,
  displayFormatter,
  ValueLabel,
  Slider,
  grabbed = false,
}: Props) => {
  const index = indexOf(value, values);
  const selectIndex = useCallback(
    (index: number) => {
      onValue?.(values[index]);
      setTimeout(() => radios.current[index]?.focus(), 0);
    },
    [onValue, values],
  );
  const radios = useRef<Record<number, HTMLDivElement>>({});
  useEffect(() => {
    const anyFocused = Object.values(radios.current).some(
      (r) => document.activeElement === r || r.contains(document.activeElement),
    );
    if (anyFocused) {
      radios.current[index ?? 0]?.focus();
    }
  }, [index]);
  return (
    <div
      style={{ display: "flex", flexDirection: "row", alignItems: "stretch" }}
    >
      <Slider
        index={index}
        count={values.length}
        selectIndex={selectIndex}
        onGrabOrRelease={onGrabOrRelease}
        grabbed={grabbed}
      />
      <LabelGroup
        accessibilityLabel={accessibilityLabel}
        value={value}
        values={values}
        displayFormatter={displayFormatter}
        valueLabel={ValueLabel}
        radios={radios}
        selectIndex={selectIndex}
      ></LabelGroup>
    </div>
  );
};

export default EnumSlider;
