import ParamKnob from "../param_components/knob";

export const Mg = () => (
  <div className="bg-zone border-border flex h-[207px] w-[103px] flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      MG
    </div>
    <div className="flex flex-row items-start px-[15px] pb-[5px] pt-[14px]">
      <ParamKnob
        param="mg_rate"
        label="rate"
        accessibilityLabel="Modulation Rate"
        style="secondary"
      />
      <div className="w-[11px]"></div>
      <ParamKnob
        param="mg_delay"
        label="delay"
        accessibilityLabel="Modulation Delay"
        style="secondary"
      />
    </div>
    <div className="flex w-full flex-row justify-center pb-[5px]">
      <ParamKnob
        param="mg_pitch"
        label="pitch"
        accessibilityLabel="Pitch Modulation"
        style="secondary"
      />
    </div>
    <div className="flex w-full flex-row justify-center">
      <ParamKnob
        param="mg_vcf"
        label="vcf"
        accessibilityLabel="Filter Modulation"
        style="secondary"
      />
    </div>
  </div>
);

export default Mg;
