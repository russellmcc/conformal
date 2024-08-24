import ParamEnumSlider from "../param_components/enum_slider";
import ParamKnob from "../param_components/knob";

export const Vca = () => (
  <div className="bg-zone border-border flex flex-col border">
    <div className="bg-border text-zone h-[15px] cursor-default select-none text-center text-sm font-bold">
      VCA
    </div>
    <div className="flex flex-row items-start px-[15px] py-[31px]">
      <div className="pt-[8px]">
        <ParamEnumSlider
          param="vca_mode"
          label="mode"
          accessibilityLabel="Amplifier Mode"
          displayFormatter={(value) => {
            if (value === "Envelope") {
              return "env";
            }
            return value.toLowerCase();
          }}
        />
      </div>
      <ParamKnob
        param="vca_velocity"
        label="vel"
        accessibilityLabel="Amplifier Velocity Sensitivity"
      />
      <div className="w-[11px]"></div>
      <ParamKnob
        param="vca_level"
        label="level"
        accessibilityLabel="Amplifier Level"
      />
    </div>
  </div>
);

export default Vca;
