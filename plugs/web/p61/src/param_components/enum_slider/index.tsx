import { useCallback, useState } from "react";
import { useEnumParam } from "plugin";
import EnumSlider, { Props } from "../../components/enum_slider";

export interface ParamEnumSliderProps {
  param: string;

  label?: string;

  accessibilityLabel?: string;

  width?: Props["width"];
  textAlign?: Props["textAlign"];

  displayFormatter?: Props["displayFormatter"];
}

export const ParamEnumSlider = ({
  param,
  label,
  accessibilityLabel,
  width,
  textAlign,
  displayFormatter,
}: ParamEnumSliderProps) => {
  const [grabbed, setGrabbed] = useState(false);
  const {
    info: { title, values, default: defaultValue },
    value,
    set,
    grab,
    release,
  } = useEnumParam(param);
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
    <EnumSlider
      label={label ?? title}
      accessibilityLabel={accessibilityLabel}
      defaultValue={defaultValue}
      value={value}
      values={values}
      onValue={(value) => {
        set(value);
      }}
      grabbed={grabbed}
      onGrabOrRelease={onGrabOrRelease}
      displayFormatter={displayFormatter ?? ((value) => value.toLowerCase())}
      width={width}
      textAlign={textAlign}
    />
  );
};

export default ParamEnumSlider;
