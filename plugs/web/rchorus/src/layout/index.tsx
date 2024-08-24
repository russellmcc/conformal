import { useSwitchParam } from "plugin";
import logo from "../assets/logo.svg";
import { forwardRef, useCallback } from "react";
import EnumSlider, { ValueLabelProps } from "music-ui/enum-slider";
import Slider from "./slider";

const Layout = () => {
  const { value: bypassed, set: setBypassed } = useSwitchParam("bypass");
  const enabled = !bypassed;
  const setEnabled = useCallback(
    (enabled: boolean) => setBypassed(!enabled),
    [setBypassed],
  );
  return (
    <div
      style={{
        position: "relative",
        width: "400px",
        height: "400px",
        whiteSpace: "pre-wrap",
        padding: "0px",
        margin: "0px",
      }}
    >
      <div
        style={{
          textAlign: "right",
          marginRight: "21px",
          paddingTop: "11px",
        }}
      >
        {"Analogue\nModeled\nChorus\nEffect"}
      </div>
      <div
        style={{
          textAlign: "right",
          marginRight: "14px",
          paddingTop: "5px",
        }}
      >
        <img src={logo} draggable={false} />
      </div>
      <div style={{ position: "absolute", bottom: "21px", left: "21px" }}>
        <div>Chorus</div>
        <div style={{ marginTop: "11px" }}>
          <EnumSlider
            values={["On", "Off"]}
            value={enabled ? "On" : "Off"}
            onValue={(v) => {
              setEnabled(v === "On");
            }}
            accessibilityLabel={"Chorus"}
            ValueLabel={forwardRef<HTMLDivElement, ValueLabelProps>(
              // eslint-disable-next-line prefer-arrow-functions/prefer-arrow-functions
              function ValueLabel({ label, ...props }, ref) {
                return (
                  <div ref={ref} {...props}>
                    {label}
                  </div>
                );
              },
            )}
            Slider={Slider}
          />
        </div>
      </div>
    </div>
  );
};

export default Layout;
