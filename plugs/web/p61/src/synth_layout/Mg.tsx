import ParamKnob from "../param_components/knob";

export const Mg = () => (
  <div className="bg-zone border-border flex flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      MG
    </div>
    <div className="flex flex-row items-start px-[15px] pt-[14px]">
      <ParamKnob
        param="mg_rate"
        label="rate"
        accessibilityLabel="Modulation Rate"
      />
      <div className="w-[11px]"></div>
      <ParamKnob
        param="mg_delay"
        label="delay"
        accessibilityLabel="Modulation Delay"
      />
    </div>
    <div className="flex flex-row items-start px-[15px] pb-4">
      <ParamKnob
        param="mg_pitch"
        label="pitch"
        accessibilityLabel="Pitch Modulation"
      />
      <div className="w-[11px]"></div>
      <ParamKnob
        param="mg_vcf"
        label="vcf"
        accessibilityLabel="Filter Modulation"
      />
    </div>
  </div>
);

export default Mg;
