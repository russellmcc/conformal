import ParamKnob from "../param_components/knob";

export const Wheel = () => (
  <div className="bg-zone border-border ms-px flex h-[207px] w-[61px] flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      WHEEL
    </div>
    <div className="flex flex-col items-start px-[15px] pt-[14px]">
      <ParamKnob
        param="wheel_rate"
        label="rate"
        accessibilityLabel="Wheel Modulation Rate"
        style="secondary"
      />
      <div className="h-[5px]"></div>
      <ParamKnob
        param="wheel_dco"
        label="pitch"
        accessibilityLabel="Wheel Modulation DCO Depth"
        style="secondary"
      />
      <div className="h-[5px]"></div>
      <ParamKnob
        param="wheel_vcf"
        label="vcf"
        accessibilityLabel="Wheel Modulation VCF Depth"
        style="secondary"
      />
    </div>
  </div>
);
