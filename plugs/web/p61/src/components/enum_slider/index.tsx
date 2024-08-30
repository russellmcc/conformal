import { forwardRef, useCallback, useMemo, useState } from "react";
import Slider from "./Slider";
import {
  ValueLabel,
  ValueLabelProps,
  EnumSlider as EnumSliderInternal,
} from "music-ui/enum-slider";

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
   * Label of the enum
   */
  label: string;

  /**
   * Accessibility label for the enum - can contain more information than `label`
   */
  accessibilityLabel?: string;

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

  /**
   * Default value to reset to on reset-to-default gesture
   */
  defaultValue?: string;

  width?: "narrow" | "normal";

  textAlign?: "start" | "end";
}

const useValueLabel = (textAlign: Props["textAlign"]): ValueLabel =>
  useMemo(() => {
    const v = forwardRef<HTMLDivElement, ValueLabelProps>(
      ({ checked, label, ...props }: ValueLabelProps, ref) => {
        const [hover, setHover] = useState(false);

        const onMouseEnter = useCallback(() => {
          setHover(true);
        }, []);
        const onMouseLeave = useCallback(() => {
          setHover(false);
        }, []);

        const pop = hover || checked;
        const popOpacity = hover && !checked ? "75%" : "100%";
        const popDuration = hover ? "duration-100" : "duration-300";
        return (
          <div
            {...props}
            ref={ref}
            onMouseEnter={onMouseEnter}
            onMouseLeave={onMouseLeave}
          >
            <div
              className={`text-border transition-opacity ${popDuration} ease-in`}
              style={{ opacity: pop ? "0%" : "100%", textAlign }}
            >
              {label}
            </div>
            <div
              className={`text-pop ${popDuration} absolute inset-0 blur-[1px] transition-opacity ease-in`}
              style={{ opacity: pop ? popOpacity : "0%", textAlign }}
            >
              {label}
            </div>
            <div
              className={`text-pop absolute inset-0 transition-opacity ${popDuration} ease-in`}
              style={{ opacity: pop ? popOpacity : "0%", textAlign }}
            >
              {label}
            </div>
          </div>
        );
      },
    );
    v.displayName = "ValueLabel";
    return v;
  }, [textAlign]);

const EnumSlider = ({
  value,
  values,
  label,
  onValue,
  onGrabOrRelease,
  displayFormatter,
  accessibilityLabel,
  width = "normal",
  textAlign = "start",
  defaultValue,
}: Props) => {
  const valueLabel = useValueLabel(textAlign);
  const onDoubleClick: React.MouseEventHandler = useCallback(
    (event) => {
      if (defaultValue !== undefined) {
        event.stopPropagation();
        event.preventDefault();
        onValue?.(defaultValue);
      }
    },
    [defaultValue, onValue],
  );
  return (
    <div
      className={`cursor-default touch-none select-none flex-row items-stretch`}
      style={{ width: width === "normal" ? "91px" : "61px" }}
    >
      <EnumSliderInternal
        value={value}
        values={values}
        accessibilityLabel={accessibilityLabel ?? label}
        displayFormatter={displayFormatter}
        Slider={Slider}
        ValueLabel={valueLabel}
        onValue={onValue}
        onGrabOrRelease={onGrabOrRelease}
      />
      <div
        className="text-border w-[31px] text-center"
        onDoubleClick={onDoubleClick}
      >
        {label}
      </div>
    </div>
  );
};

export default EnumSlider;
