import { Info, useNumericParam } from "@conformal/plugin";
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
import { Provider } from "@conformal/plugin";
createRoot(document.getElementById("root")!).render(
  <Provider>
    <DisplayGain />
  </Provider>,
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
  <Provider mockInfos={mockInfos}>
    <DisplayGain />
  </Provider>,
);
//#endregion mocking-parameters
