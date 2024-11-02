import { ConfigMetadata } from "@conformal/stamp";
export type PlugType = "Effect" | "Synth";
import uuidHex from "./uuid";
import path from "node:path";

export type Config = {
  plug_type: string;
  plug_slug: string;
  plug_name: string;
  vendor_name: string;
};

// Update the rust versions by grabbing our own package.json,
// This is valid because we version all crates and packages together.
// eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
const version: string =
  // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
  (await Bun.file(path.join(__dirname, "..", "package.json")).json()).version;

export const toEnv = (
  config: Config,
  {
    skipTodo,
    component_crate_version,
    vst_crate_version,
  }: {
    skipTodo?: boolean;
    component_crate_version?: string;
    vst_crate_version?: string;
  },
): Record<string, string> => ({
  ...config,
  class_id: uuidHex(),
  edit_class_id: uuidHex(),
  gitignore: ".gitignore",
  component_crate_version: component_crate_version ?? version,
  vst_crate_version: vst_crate_version ?? version,
  task_marker: (skipTodo ?? false) ? "DONE" : "TOD" + "O",
});

export const metadatas: Record<keyof Config, ConfigMetadata> = {
  plug_type: {
    prompt: "Plug-in type",
    description: "The type of plug-in to create ('Effect' or 'Synth')",
    default: "Effect",
  },
  plug_slug: {
    prompt: "Plug-in slug (lower snake_case, e.g. `my_plugin`)",
    description:
      "The name of the first plug-in in lower snake_case, e.g. `my_plugin`",
    default: "my_plugin",
  },
  vendor_name: {
    prompt:
      'Human-readable vendor name (DAWs often present plug-ins grouped by vendor).  e.g., "My Project"?',
    description:
      "Human-readable vendor name, e.g. `My Project`. DAWs often present plug-ins grouped by vendor",
    default: "My Project",
  },
  plug_name: {
    prompt: "Human-readable plug-in name (e.g. `My Plug-in`)?",
    description: "Human-readable vendor name, e.g. `My Plug-in`",
    default: "My Plug-in",
  },
};
