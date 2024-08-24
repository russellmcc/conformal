import ParamKnob from "../param_components/knob";
import { exponentialScale } from "../scale";

export const Env = () => (
  <div className="bg-zone border-border flex flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      ENV
    </div>
    <div className="flex flex-row items-start">
      <div className="flex flex-col items-end px-[15px] pb-4 pt-[15px]">
        <ParamKnob
          param="attack"
          label="attack"
          accessibilityLabel="Attack Time"
          scale={exponentialScale(0.5, 0.1)}
        />
        <ParamKnob
          param="decay"
          label="decay"
          accessibilityLabel="Decay Time"
          scale={exponentialScale(0.5, 0.1)}
        />
        <ParamKnob
          param="sustain"
          label="sustain"
          accessibilityLabel="Sustain"
        />
        <ParamKnob
          param="release"
          label="release"
          accessibilityLabel="Release Time"
          scale={exponentialScale(0.5, 0.1)}
        />
      </div>
    </div>
  </div>
);

export default Env;
