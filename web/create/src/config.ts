import { Command } from "@commander-js/extra-typings";
import { input } from "@inquirer/prompts";
import uuidHex from "./uuid";

export type Config = {
  proj_slug: string;
  plug_slug: string;
  plug_name: string;
  vendor_name: string;
};

type ConfigMetadata = {
  key: keyof Config;
  prompt: string;
  description: string;
  default?: string;
};

export const toEnv = (
  config: Config,
  {
    skipTodo,
    component_crate_version,
    vst_crate_version,
  }: {
    skipTodo?: boolean;
    component_crate_version: string;
    vst_crate_version: string;
  },
): Record<string, string> => ({
  ...config,
  class_id: uuidHex(),
  edit_class_id: uuidHex(),
  gitignore: ".gitignore",
  component_crate_version,
  vst_crate_version,
  task_marker: (skipTodo ?? false) ? "DONE" : "TOD" + "O",
});

const metadatas: ConfigMetadata[] = [
  {
    key: "proj_slug",
    prompt: "Project slug (lower snake_case, e.g. `my_project`)",
    description: "Slug for the project in lower snake_case, e.g. `my_project`",
    default: "my_project",
  },
  {
    key: "plug_slug",
    prompt: "Plug-in slug (lower snake_case, e.g. `my_plugin`)",
    description:
      "The name of the first plug-in in lower snake_case, e.g. `my_plugin`",
    default: "my_plugin",
  },
  {
    key: "vendor_name",
    prompt:
      'Human-readable vendor name (DAWs often present plug-ins grouped by vendor).  e.g., "My Project"?',
    description:
      "Human-readable vendor name, e.g. `My Project`. DAWs often present plug-ins grouped by vendor",
    default: "My Project",
  },
  {
    key: "plug_name",
    prompt: "Human-readable plug-in name (e.g. `My Plug-in`)?",
    description: "Human-readable vendor name, e.g. `My Plug-in`",
    default: "My Plug-in",
  },
];

const fromCli = async (argv?: readonly string[]): Promise<Partial<Config>> => {
  const command = new Command();
  for (const { key, description } of metadatas) {
    if (key !== "proj_slug") {
      command.option(`--${key} <${key}>`, description);
    } else {
      // Name is a positional argument
      command.argument("<proj_slug>", description);
    }
  }
  const processed = await command.parseAsync(argv);

  const ret = Object.fromEntries(
    Object.entries(processed.opts()).filter(([_, v]) => v !== undefined),
  );
  if (processed.args.length > 0) {
    ret.proj_slug = processed.args[0];
  }
  return ret;
};

const doPrompt = async (metadata: ConfigMetadata): Promise<string> =>
  input({ message: metadata.prompt, default: metadata.default });

const promptRemainder = async (config: Partial<Config>): Promise<Config> => {
  const ret: Partial<Config> = { ...config };
  for (const metadata of metadatas) {
    if (metadata.key in ret) {
      continue;
    }
    ret[metadata.key] = await doPrompt(metadata);
  }
  // Note that we've filled in all of config here, since metadatas must contain all configs!
  // It would be cool to check this in ts but I don't know how
  return ret as Config;
};

export const getConfig = async (argv?: readonly string[]): Promise<Config> =>
  await promptRemainder(await fromCli(argv));
