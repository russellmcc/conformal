import { Info } from "@conformal/plugin";

const infos = new Map<string, Info>(
  Object.entries({
    bypass: {
      title: "Bypass",
      type_specific: {
        t: "switch",
        default: false,
      },
    },
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

export default infos;
