import { CommandLineAction } from "@rushstack/ts-command-line";
import { $ } from "bun";
import { appendFile } from "node:fs/promises";

const RUST_VERSION = "1.79.0";
const VST3_VERSION = "v3.7.8_build_34";

interface Tool {
  name: string;
  check(): Promise<boolean>;
  install(): Promise<void>;
}

// Note that we work around some odd ld behavior by using `lld`
const brew = (name: string): Tool => ({
  name,
  check: async () =>
    (await $`command -v ${name}`.nothrow().quiet()).exitCode == 0,
  install: async () => {
    await $`brew install ${name}`;
  },
});

const rustup = () => brew("rustup");

const rust = (): Tool => ({
  name: "rust",
  check: async () => {
    try {
      const cargoVersion = (await $`cargo --version`.text()).split(" ")[1];
      if (cargoVersion !== RUST_VERSION) {
        console.warn(`cargo installed, but wrong version ${cargoVersion}`);
        return false;
      }
      return true;
    } catch (e) {
      return false;
    }
  },
  install: async () => {
    await $`rustup toolchain install ${RUST_VERSION}`;
    await $`rustup default ${RUST_VERSION}`;
    await $`rustup target add x86_64-apple-darwin`;
    await $`rustup component add rustfmt`;
    await $`rustup component add clippy`;
    await $`cargo install --locked cargo-about`;
  },
});

const rustNightly = (): Tool => ({
  name: "rust-nightly",
  check: async () =>
    (await $`cargo +nightly --version`.quiet().nothrow()).exitCode === 0,
  install: async () => {
    await $`rustup toolchain install nightly`;
    await $`rustup +nightly component add miri`;
  },
});

const lld = (): Tool => ({
  name: "lld",
  check: async () =>
    (
      await $`command -v /opt/homebrew/Cellar/llvm@17/17.0.6/bin/ld64.lld`
        .nothrow()
        .quiet()
    ).exitCode == 0,
  install: async () => {
    await $`brew install llvm@17`;
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
    const buildPath = `${process.env.VST3_SDK_DIR}/build`;
    await $`mkdir -p ${buildPath}`.quiet();
    await $`cmake ..`.cwd(buildPath);
    await $`cmake --build . --target validator`.cwd(buildPath);
  },
});

const cmake = () => brew("cmake");

export const bootstrap = async (): Promise<void> => {
  const tools: Tool[] = [
    vst3(),
    cmake(),
    vst3Validator(),
    rustup(),
    rustNightly(),
    rust(),
    lld(),
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

export class BootstrapAction extends CommandLineAction {
  public constructor() {
    super({
      actionName: "bootstrap",
      summary: "Make sure all build requirements are installed",
      documentation: "",
    });
  }

  public async onExecute(): Promise<void> {
    await bootstrap();
  }
}
