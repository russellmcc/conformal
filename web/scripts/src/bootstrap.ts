import { $ } from "bun";
import { appendFile } from "node:fs/promises";
import { Command } from "@commander-js/extra-typings";

const RUST_VERSION = "1.90.0";
const CARGO_ABOUT_VERSION = "0.6.6";
const VST3_VERSION = "v3.7.14_build_55";

type Tool = {
  name: string;
  check(): Promise<boolean>;
  install(): Promise<void>;
};

const brew = (name: string, command?: string): Tool => ({
  name,
  check: async () =>
    (await $`command -v ${command ?? name}`.nothrow().quiet()).exitCode == 0,
  install: async () => {
    await $`brew install ${name}`;
  },
});

const rustup = () => brew("rustup");

const knope = () => brew("knope-dev/tap/knope", "knope");

const checkAvailable = async (
  command: string,
  label: string,
  required: string[],
) => {
  const installeds = (await $`${{ raw: command }}`.text())
    .split("\n")
    .map((x) => x.trim());
  for (const r of required) {
    if (!installeds.some((x) => new RegExp(r).test(x))) {
      console.warn(`missing ${label} ${r}, only have ${installeds.join(", ")}`);
      return false;
    }
  }
  return true;
};

const rust = (options: {
  rustVersion?: string;
  cargoAboutVersion?: string;
}): Tool => {
  const version = options.rustVersion ?? RUST_VERSION;
  const cargoAboutVersion = options.cargoAboutVersion ?? CARGO_ABOUT_VERSION;
  return {
    name: "rust",
    check: async () => {
      try {
        const checkCargoToolVersion = async (
          command: string,
          expectedVersion: string,
          toolName = "cargo",
        ): Promise<boolean> => {
          const installedVersion = (await $`${{ raw: command }}`.text())
            .split(" ")[1]
            ?.trim();
          if (installedVersion !== expectedVersion) {
            console.warn(
              installedVersion
                ? `${toolName} installed, but wrong version ${installedVersion} (expected ${expectedVersion})`
                : `${toolName} installed, but could not get version`,
            );
            return false;
          }
          return true;
        };

        if (!(await checkCargoToolVersion("cargo --version", version))) {
          return false;
        }

        if (
          !(await checkAvailable("rustup target list --installed", "target", [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
          ]))
        ) {
          return false;
        }

        if (
          !(await checkAvailable(
            "rustup component list --installed",
            "component",
            [
              "rustfmt-(?:aarch64|x86_64)-apple-darwin",
              "clippy-(?:aarch64|x86_64)-apple-darwin",
            ],
          ))
        ) {
          return false;
        }

        if (
          !(await checkCargoToolVersion(
            "cargo about --version",
            cargoAboutVersion,
            "cargo-about",
          ))
        ) {
          return false;
        }

        return true;
      } catch {
        return false;
      }
    },
    install: async () => {
      await $`rustup toolchain install ${version}`;
      await $`rustup default ${version}`;
      await $`rustup target add x86_64-apple-darwin`;
      await $`rustup component add rustfmt`;
      await $`rustup component add clippy`;
      await $`cargo install --locked cargo-about --version ${cargoAboutVersion}`;
    },
  };
};

const rustNightly = (): Tool => ({
  name: "rust-nightly",
  check: async () => {
    if ((await $`cargo +nightly --version`.quiet().nothrow()).exitCode !== 0) {
      console.warn("missing rust-nightly");
      return false;
    }
    // Check if miri is a component of nightly
    return await checkAvailable(
      "rustup +nightly component list --installed",
      "component",
      ["miri-(?:aarch64|x86_64)-apple-darwin"],
    );
  },
  install: async () => {
    await $`rustup toolchain install nightly`;
    await $`rustup +nightly component add miri`;
  },
});

const vst3 = (options: { vstSdkVersion?: string }): Tool => ({
  name: "vst3",
  // eslint-disable-next-line @typescript-eslint/require-await
  check: async (): Promise<boolean> => process.env.VST3_SDK_DIR !== undefined,
  install: async () => {
    const vstSdkVersion = options.vstSdkVersion ?? VST3_VERSION;
    const tmpDir = (await $`mktemp -d`.text()).trim();
    await $`git clone https://github.com/steinbergmedia/vst3sdk.git --branch ${vstSdkVersion}`.cwd(
      tmpDir,
    );
    await $`git submodule update --init --recursive`.cwd(`${tmpDir}/vst3sdk`);
    process.env.VST3_SDK_DIR = `${tmpDir}/vst3sdk`;
    await appendFile(".env", `VST3_SDK_DIR=${process.env.VST3_SDK_DIR}\n`);
  },
});

const vst3Validator = (): Tool => ({
  name: "vst3 validator",
  check: async () =>
    (
      await $`command -v ${process.env.VST3_SDK_DIR}/build/bin/validator`
        .nothrow()
        .quiet()
    ).exitCode == 0,

  install: async () => {
    // use cmake to build the validator
    const sdk = process.env.VST3_SDK_DIR;
    if (!sdk) {
      throw new Error("VST3_SDK_DIR is not set");
    }
    const buildPath = `${sdk}/build`;
    await $`mkdir -p ${buildPath}`.quiet();
    await $`cmake ..`.cwd(buildPath);
    await $`cmake --build . --target validator`.cwd(buildPath);
  },
});

const cmake = () => brew("cmake");

export type BootstrapOptions = {
  rustVersion?: string;
  cargoAboutVersion?: string;
  vstSdkVersion?: string;
};

export const bootstrap = async (
  options: BootstrapOptions = {},
): Promise<void> => {
  const tools: Tool[] = [
    vst3({ vstSdkVersion: options.vstSdkVersion }),
    knope(),
    cmake(),
    vst3Validator(),
    rustup(),
    rustNightly(),
    rust({
      rustVersion: options.rustVersion,
      cargoAboutVersion: options.cargoAboutVersion,
    }),
  ];
  for (const tool of tools) {
    console.log(`Checking ${tool.name}...`);
    if (!(await tool.check())) {
      console.log(`Missing. Installing ${tool.name}`);
      await tool.install();
      if (!(await tool.check())) {
        console.error(`Failed to install ${tool.name}`);
        process.exit(1);
      }
    } else {
      console.log(`Found ${tool.name}`);
    }
  }
};

export const addBootstrapCommand = (command: Command) =>
  command
    .command("bootstrap")
    .description("Make sure all build requirements are installed")
    .option(
      "--rust-version <version>",
      `The version of Rust to use (default: ${RUST_VERSION})`,
    )
    .option(
      "--cargo-about-version <version>",
      `The version of cargo-about to use (default: ${CARGO_ABOUT_VERSION})`,
    )
    .option(
      "--vst-sdk-version <version>",
      `The version of the VST SDK to use (default: ${VST3_VERSION})`,
    )
    .action(async (options) => {
      await bootstrap(options);
    });
