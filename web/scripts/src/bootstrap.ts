import { $ } from "bun";
import { appendFile } from "node:fs/promises";
import { Command } from "@commander-js/extra-typings";

const RUST_VERSION = "1.84.1";
const CARGO_ABOUT_VERSION = "0.6.6";
const VST3_VERSION = "v3.7.8_build_34";

type Tool = {
  name: string;
  check(): Promise<boolean>;
  install(): Promise<void>;
};

const brew = (name: string): Tool => ({
  name,
  check: async () =>
    (await $`command -v ${name}`.nothrow().quiet()).exitCode == 0,
  install: async () => {
    await $`brew install ${name}`;
  },
});

const rustup = () => brew("rustup");

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
        const cargoVersion = (await $`cargo --version`.text()).split(" ")[1];
        if (cargoVersion !== version) {
          console.warn(
            cargoVersion
              ? `cargo installed, but wrong version ${cargoVersion}`
              : "cargo installed, but could not get version",
          );
          return false;
        }
        return true;
      } catch (e) {
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
  check: async () =>
    (await $`cargo +nightly --version`.quiet().nothrow()).exitCode === 0,
  install: async () => {
    await $`rustup toolchain install nightly`;
    await $`rustup +nightly component add miri`;
  },
});

const vst3 = (): Tool => ({
  name: "vst3",
  // eslint-disable-next-line @typescript-eslint/require-await
  check: async (): Promise<boolean> => process.env.VST3_SDK_DIR !== undefined,
  install: async () => {
    const tmpDir = (await $`mktemp -d`.text()).trim();
    await $`git clone https://github.com/steinbergmedia/vst3sdk.git --branch ${VST3_VERSION}`.cwd(
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
};

export const bootstrap = async (
  options: BootstrapOptions = {},
): Promise<void> => {
  const tools: Tool[] = [
    vst3(),
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
    .option("--rust-version <version>", "The version of Rust to use")
    .option(
      "--cargo-about-version <version>",
      "The version of cargo-about to use",
    )
    .action(async (options) => {
      await bootstrap(options);
    });
