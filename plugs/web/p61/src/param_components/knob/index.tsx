import { useCallback, useState } from "react";
import { useNumericParam } from "@conformal/plugin";
import Knob, { Props } from "../../components/knob";
import { Scale } from "../../scale";

export interface ParamKnobProps {
  param: string;

  label?: string;

  accessibilityLabel?: string;

  style?: Props["style"];

  scale?: Scale;
}

export const ParamKnob = ({
  param,
  label,
  accessibilityLabel,
  style,
  scale,
}: ParamKnobProps) => {
  const [grabbed, setGrabbed] = useState(false);
  const {
    info: {
      title,
      valid_range: [min_value, max_value],
      default: defaultValue,
      units,
    },
    value,
    set,
    grab,
    release,
  } = useNumericParam(param);
  let scaled = ((value - min_value) / (max_value - min_value)) * 100;
  if (scale) {
    scaled = scale.from(scaled / 100) * 100;
  }
  const unscale = useCallback(
    (scaled: number) => {
      let unscaledValue = Math.min(Math.max(scaled / 100, 0.0), 1.0);
      if (scale) {
        unscaledValue = scale.to(unscaledValue);
      }

      return unscaledValue * (max_value - min_value) + min_value;
    },
    [max_value, min_value, scale],
  );
  const onGrabOrRelease = useCallback(
    (grabbed: boolean) => {
      setGrabbed(grabbed);
      if (grabbed) {
        grab();
      } else {
        release();
      }
    },
    [grab, release, setGrabbed],
  );
  return (
    <Knob
      label={label ?? title}
      value={scaled}
      onValue={(scaled) => {
        set(unscale(scaled));
      }}
      grabbed={grabbed}
      onGrabOrRelease={onGrabOrRelease}
      valueFormatter={
        units ? (value) => `${unscale(value).toFixed(0)}${units}` : undefined
      }
      accessibilityLabel={accessibilityLabel}
      style={style}
      defaultValue={defaultValue}
    />
  );
};

export default ParamKnob;
