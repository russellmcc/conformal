import { Info } from "plugin";

const infos = new Map<string, Info>(
  Object.entries({
    bypass: {
      title: "Bypass",
      type_specific: {
        t: "switch",
        default: false,
      },
    },
  }),
);

export default infos;
