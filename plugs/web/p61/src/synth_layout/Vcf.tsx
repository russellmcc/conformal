import ParamKnob from "../param_components/knob";

export const Vcf = () => (
  <div className="bg-zone border-border mr-px flex flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      VCF
    </div>
    <div className="flex flex-row items-start">
      <div className="flex flex-row items-start py-[31px] pl-[15px]">
        <ParamKnob
          param="vcf_cutoff"
          label="cutoff"
          accessibilityLabel="Filter Cutoff Frequency"
        />
        <div className="w-[11px]"></div>
        <ParamKnob
          param="vcf_resonance"
          label="res"
          accessibilityLabel="Filter Resonance"
        />
      </div>
      <div className="flex flex-col items-start px-[15px] pb-[8px] pt-[21px]">
        <div className="flex flex-row pb-[5px]">
          <ParamKnob
            param="vcf_tracking"
            label="key"
            accessibilityLabel="Filter Keyboard Tracking"
            style="secondary"
          />
          <div className="w-[7px]"></div>
          <ParamKnob
            param="vcf_env"
            label="env"
            accessibilityLabel="Filter Envelope Amount"
            style="secondary"
          />
        </div>
        <div className="flex w-full flex-row justify-center">
          <ParamKnob
            param="vcf_velocity"
            label="vel"
            accessibilityLabel="Filter Velocity Sensitivity"
            style="secondary"
          />
        </div>
      </div>
    </div>
  </div>
);

export default Vcf;
