import ParamEnumSlider from "../param_components/enum_slider";
import ParamKnob from "../param_components/knob";

export const Dco2 = () => (
  <div className="border-border mr-px flex flex-col border-t">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      DCO2
    </div>
    <div className="flex flex-row items-start">
      <div className="bg-zone border-border z-10 -mr-px flex flex-row items-end border-b border-l pb-4 pl-[15px] pt-[31px]">
        <ParamEnumSlider
          param="dco2_shape"
          label="shape"
          accessibilityLabel="DCO2 Shape"
        />
        <div className="w-[15px] shrink" />
        <ParamKnob
          param="dco2_detune"
          label="detune"
          accessibilityLabel="DCO2 Detune"
        />
        <div className="w-[15px] shrink" />
        <ParamEnumSlider
          param="dco2_octave"
          label="octave"
          accessibilityLabel="DCO2 Octave"
        />
      </div>
      <div className="bg-zone border-border relative ml-[-30px] flex flex-row border-x border-b pb-4 pl-[30px] pr-[15px] pt-[31px]">
        {/* This is a hacky way to hide the border overlap */}
        <div className="bg-zone absolute bottom-0 left-0 z-20 h-[80px] w-[30px]"></div>
        <ParamEnumSlider
          param="dco2_interval"
          label="interval"
          accessibilityLabel="DCO2 Interval"
          width="narrow"
          textAlign="end"
        />
      </div>
    </div>
  </div>
);

export default Dco2;
