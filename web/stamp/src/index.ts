import { promises as fs } from "node:fs";
import path from "node:path";
import { compile } from "handlebars";
import { Command } from "@commander-js/extra-typings";
import { input } from "@inquirer/prompts";

export type ConfigMetadata = {
  prompt: string;
  positional?: boolean;
  description: string;
  default?: string;
};

const dirExists = async (dir: string) => {
  try {
    await fs.readdir(dir);
    return true;
  } catch {
    return false;
  }
};

export type StampOptions = {
  merge?: boolean;
};

/**
 * Stamps a template directory into a destination directory.
 *
 * @param dest - The destination directory.
 * @param templateDir - The template directory.
 * @param env - environment variables to use in the template.
 * @param options - Options.
 * @param options.merge - If true, the template will be merged with the destination directory.
 *
 */
export const stampTemplate = async (
  dest: string,
  templateDir: string,
  env: Record<string, string>,
  options: StampOptions = {},
) => {
  if (!options.merge && (await dirExists(dest))) {
    throw new Error(`Directory already exists: ${dest}`);
  }

  const files = await fs.readdir(templateDir, {
    recursive: true,
    withFileTypes: true,
  });
  for (const file of files) {
    const srcPath = path.join(file.path, file.name);
    const destPath = path.join(
      dest,
      compile(path.relative(templateDir, srcPath))(env),
    );

    if (file.isDirectory()) {
      continue;
    }

    if (await Bun.file(destPath).exists()) {
      throw new Error(`File already exists: ${destPath}`);
    }

    await fs.mkdir(path.dirname(destPath), { recursive: true });
    const srcContent = await Bun.file(srcPath).text();
    await Bun.write(destPath, compile(srcContent)(env));
  }
};

const doPrompt = async (metadata: ConfigMetadata): Promise<string> =>
  input({ message: metadata.prompt, default: metadata.default });

const promptRemainder = async <K extends string>(
  config: Partial<{ [k in K]: string }>,
  metadatas: Record<K, ConfigMetadata>,
): Promise<{ [k in K]: string }> => {
  const ret: Partial<{ [k in K]: string }> = { ...config };
  for (const key in metadatas) {
    if (key in ret) {
      continue;
    }
    ret[key] = await doPrompt(metadatas[key]);
  }
  // Note that we've filled in all of config here, since metadatas must contain all configs!
  // It would be cool to check this in ts but I don't know how
  return ret as { [k in K]: string };
};

export const buildStampCommand = <K extends string>({
  command,
  metadatas,
  toEnv,
  toDest,
  toTemplate,
  postBuild,
  options,
}: {
  command: Command;
  metadatas: Record<K, ConfigMetadata>;
  toEnv: (config: { [k in K]: string }) => Promise<Record<string, string>>;
  toDest: (config: { [k in K]: string }) => Promise<string>;
  toTemplate: (config: { [k in K]: string }) => Promise<string>;
  postBuild?: (config: { [k in K]: string }) => Promise<void>;
  options?: StampOptions;
}): Command => {
  const positionals: K[] = [];
  for (const key in metadatas) {
    const metadata = metadatas[key];
    if (metadata.positional) {
      command.argument(`<${key}>`, metadata.description);
      positionals.push(key);
    } else {
      command.option(`--${key} <${key}>`, metadata.description);
    }
  }
  command.action(async (optsRaw, command) => {
    const opts = optsRaw as Partial<Record<K, string | undefined>>;
    const configPartial: Partial<{ [k in K]: string }> = {};
    for (const key in opts) {
      const opt = opts[key];
      if (opt !== undefined) {
        configPartial[key] = opt;
      }
    }
    for (const [argIndex, arg] of command.args.entries()) {
      const key = positionals[argIndex];
      if (key !== undefined) {
        configPartial[key] = arg;
      }
    }
    const config = await promptRemainder(configPartial, metadatas);
    const env = await toEnv(config);
    const dest = await toDest(config);
    const template = await toTemplate(config);
    await stampTemplate(dest, template, env, options);
    if (postBuild) {
      await postBuild(config);
    }
  });
  return command;
};
