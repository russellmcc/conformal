import ParamEnumSlider from "../param_components/enum_slider";
import ParamKnob from "../param_components/knob";

export const Dco1 = () => (
  <div className="bg-zone border-border mr-px flex flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      DCO1
    </div>
    <div className="flex flex-row items-center px-[15px] pb-4 pt-[31px]">
      <div className="flex flex-row items-end">
        <ParamEnumSlider
          param="dco1_shape"
          label="shape"
          accessibilityLabel="DCO1 Shape"
        />
        <div className="w-[15px] shrink" />
        <ParamKnob
          param="dco1_width"
          label="width"
          accessibilityLabel="DCO1 Width"
        />
        <div className="w-[15px] shrink" />
        <ParamEnumSlider
          param="dco1_octave"
          label="octave"
          accessibilityLabel="DCO1 Octave"
        />
      </div>
    </div>
  </div>
);

export default Dco1;
