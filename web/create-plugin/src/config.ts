export type PlugType = "Effect" | "Synth";
import uuidHex from "./uuid";
import path from "node:path";
import { parse } from "smol-toml";
import { z } from "zod";

const rustVersionSchema = z.record(z.string(), z.string());

export type Config = {
  plug_type: string;
  plug_slug: string;
  plug_name: string;
  vendor_name: string;
};

export type RustVersionMode = "real" | "mock" | "none";

export const toEnv = async (
  config: Config,
  options: { rustVersionMode: RustVersionMode } = {
    rustVersionMode: "real",
  },
): Promise<Record<string, string>> => {
  // Parse the included rust_versions.toml file
  const rustVersions: Record<string, string> =
    options.rustVersionMode === "real"
      ? rustVersionSchema.parse(
          parse(
            await Bun.file(
              path.join(__dirname, "..", "rust_versions.toml"),
            ).text(),
          ),
        )
      : options.rustVersionMode === "mock"
        ? {
            conformal_component_version: "0.0.0",
            conformal_vst_wrapper_version: "0.0.0",
            conformal_poly_version: "0.0.0",
          }
        : {};

  return {
    ...config,
    ...rustVersions,
    class_id: uuidHex(),
    edit_class_id: uuidHex(),
    gitignore: ".gitignore",
    cargo_toml: "Cargo.toml",
    task_marker: "TOD" + "O",
    test_marker: "test",
  };
};

export const metadatas = {
  plug_type: {
    prompt: "Plug-in type (`effect` or `synth`)",
    description: "The type of plug-in to create ('effect' or 'synth')",
    default: "effect",
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
