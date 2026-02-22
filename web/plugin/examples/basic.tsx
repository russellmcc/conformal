import * as Jotai from "jotai";
import { Info, Provider, useNumericParam } from "@conformal/plugin";
import { createRoot } from "react-dom/client";

//#region hook-usage
const DisplayGain = () => {
  const { value } = useNumericParam("gain");
  return <span>{value}</span>;
};
//#endregion hook-usage

//#region write-hook-usage
const ResetGainButton = () => {
  const { set } = useNumericParam("gain");
  return (
    <button
      onClick={() => {
        set(0);
      }}
    >
      Set Gain
    </button>
  );
};
//#endregion write-hook-usage

void ResetGainButton;

//#region required-providers
createRoot(document.getElementById("root")!).render(
  <Jotai.Provider>
    <Provider>
      <DisplayGain />
    </Provider>
  </Jotai.Provider>,
);
//#endregion required-providers

//#region mocking-parameters
const mockInfos = new Map<string, Info>(
  Object.entries({
    gain: {
      title: "Gain",
      type_specific: {
        t: "numeric",
        default: 100,
        valid_range: [0, 100],
        units: "%",
      },
    },
  }),
);

createRoot(document.getElementById("root")!).render(
  <Jotai.Provider>
    <Provider mockInfos={mockInfos}>
      <DisplayGain />
    </Provider>
  </Jotai.Provider>,
);
//#endregion mocking-parameters
