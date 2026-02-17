import { $ } from "bun";
import { appendFile, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Command } from "@commander-js/extra-typings";
import runShell from "./runShell";

const RUST_VERSION = "1.93.1";
const CARGO_ABOUT_VERSION = "0.6.6";
const VST3_VERSION = "v3.8.0_build_66";

type Tool = {
  name: string;
  check(): Promise<boolean>;
  install(): Promise<void>;
};

const which = async (command: string) =>
  (await $`which ${command}`.nothrow().quiet()).exitCode == 0;

const manualStep =
  ({
    name,
    step,
    forceRestart,
  }: {
    name: string;
    step: string;
    forceRestart?: boolean;
  }) =>
  async () => {
    if (
      !process.stdout.isTTY ||
      process.env.TERM === "dumb" ||
      "CI" in process.env
    ) {
      throw new Error(
        `could not find ${name}, which must be manually installed (${step})`,
      );
    }
    process.stdout.write(
      `${name} must be manually installed. Please ${step} and then press enter.`,
    );
    for await (const _ of console) {
      break;
    }
    if (forceRestart) {
      throw new Error("Please restart your terminal and run bootstrap again");
    }
  };

const installInstructionsPrompt = (url: string) =>
  `follow installation instructions at ${url}`;

const installInstructions = ({
  name,
  url,
  forceRestart,
}: {
  name: string;
  url: string;
  forceRestart?: boolean;
}) => manualStep({ name, step: installInstructionsPrompt(url), forceRestart });

const installBrew = (): Tool => ({
  name: `brew`,
  check: async () => await which("brew"),
  install: installInstructions({ name: "brew", url: "https://brew.sh/" }),
});

const installGit = (): Tool => ({
  name: "git",
  check: async () => await which("git"),
  install: installInstructions({
    name: "git",
    url: "https://git-scm.com/",
    forceRestart: true,
  }),
});

type BrewOptions = {
  command?: string;
  windows: string | undefined;
};

const brew = (name: string, options: BrewOptions): Tool => ({
  name,
  check: async () => await which(options.command ?? name),
  install:
    process.platform === "win32"
      ? async () => {
          if (options.windows) {
            await manualStep({
              name,
              step: options.windows,
              forceRestart: true,
            })();
          } else {
            throw new Error(`Cannot install necessary tool ${name} on windows`);
          }
        }
      : async () => {
          await $`brew install ${name}`;
        },
});

const rustup = () =>
  brew("rustup", {
    windows: installInstructionsPrompt(
      "https://rust-lang.org/tools/install/#rustup",
    ),
  });

const binstall = (name: string): Tool => ({
  name,
  check: async () => await which(name),
  install: async () => {
    await $`cargo binstall ${name} --no-confirm`;
  },
});

const knope = () => {
  if (process.platform === "win32") {
    return binstall("knope");
  } else {
    return brew("knope-dev/tap/knope", {
      command: "knope",
      windows: undefined,
    });
  }
};

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

const rustTargets = () =>
  process.platform === "win32"
    ? []
    : ["aarch64-apple-darwin", "x86_64-apple-darwin"];

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
          const result = await $`${{ raw: command }}`.nothrow().quiet();
          if (result.exitCode !== 0) {
            console.warn(
              `\`${command}\` failed (exit code ${result.exitCode}): ${result.stderr.toString().trim() || result.stdout.toString().trim() || "(no output)"}`,
            );
            return false;
          }
          const installedVersion = result.text().split(" ")[1]?.trim();
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
          !(await checkAvailable(
            "rustup target list --installed",
            "target",
            rustTargets(),
          ))
        ) {
          return false;
        }

        if (
          !(await checkAvailable(
            "rustup component list --installed",
            "component",
            ["rustfmt", "clippy"],
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
      for (const target of rustTargets()) {
        await $`rustup target add ${target}`;
      }
      await $`rustup component add rustfmt`;
      await $`rustup component add clippy`;
      // --force is needed because Swatinem/rust-cache removes cached binaries
      // before saving, so stale metadata in .crates2.json can trick binstall
      // into thinking cargo-about is installed when the binary is actually gone.
      // See: https://github.com/Swatinem/rust-cache/blob/779680da715d629ac1d338a641029a2f4372abb5/src/cleanup.ts#L99-L112
      await $`cargo binstall --no-confirm --force cargo-about@${cargoAboutVersion}`;
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
      ["miri"],
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
    const tmpDir = await mkdtemp(join(tmpdir(), "vst3-"));
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
  check: async () => {
    if (!("VST3_SDK_DIR" in process.env)) {
      console.log("VST SDK not detected, skipping vst3 validator");
      return true;
    }
    const outputDir = process.platform === "win32" ? "bin/Debug" : "bin";
    return await which(
      `${process.env.VST3_SDK_DIR}/build/${outputDir}/validator`,
    );
  },
  install: async () => {
    // use cmake to build the validator
    const sdk = process.env.VST3_SDK_DIR;
    if (!sdk) {
      throw new Error("VST3_SDK_DIR is not set");
    }
    const buildPath = `${sdk}/build`;
    await $`mkdir -p ${buildPath}`.quiet();
    await $`cmake .. -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF`.cwd(buildPath);
    await $`cmake --build . --target validator`.cwd(buildPath);
  },
});

const cargoBinstall = (): Tool => ({
  name: "cargo-binstall",
  check: async () => await which("cargo-binstall"),
  install: async () => {
    if (process.platform === "win32") {
      await runShell([
        "powershell",
        "-c",
        "Set-ExecutionPolicy Unrestricted -Scope Process; irm 'https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.ps1' | iex",
      ]);
      return;
    }
    await $`curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash`;
  },
});

const cmake = () => ({
  name: "cmake",
  check: async () => {
    if (!("VST3_SDK_DIR" in process.env)) {
      console.log("No VST SDK detected, so skipping cmake.");
      return true;
    }
    return await which("cmake");
  },
  install: async () => {
    if (process.platform === "win32") {
      await installInstructions({
        name: "cmake",
        url: "https://cmake.org/download/",
        forceRestart: true,
      })();
      return;
    }
    await $`brew install cmake`;
  },
});

export type BootstrapOptions = {
  rustVersion?: string;
  cargoAboutVersion?: string;
  vstSdkVersion?: string;
  withMiri?: true;
  withVstSdk?: true;
};

export const bootstrap = async (
  options: BootstrapOptions = {},
): Promise<void> => {
  const tools: Tool[] = [
    ...(process.platform === "win32" && options.withVstSdk
      ? [installGit()]
      : []),
    ...(process.platform === "win32" ? [] : [installBrew()]),
    ...(options.withVstSdk
      ? [vst3({ vstSdkVersion: options.vstSdkVersion })]
      : []),
    rustup(),
    cargoBinstall(),

    knope(),

    cmake(),
    vst3Validator(),

    ...(options.withMiri ? [rustNightly()] : []),
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
    .option(
      "--with-miri",
      "Install the Rust nightly toolchain with [miri](https://github.com/rust-lang/miri) undefined behavior checker. This can optionally be used with the `ci` command to run tests with miri.",
    )
    .option(
      "--with-vst-sdk",
      "Install the VST SDK to allow validating VST plug-ins",
    )
    .action(async (options) => {
      await bootstrap(options);
    });

/**
 * Install only the Rust toolchain. this is helpful in CI, because we can't
 * start the rust cache until we have selected our rust version.
 */
export const bootstrapRustToolchain = async (options: {
  rustVersion?: string;
}) => {
  const version = options.rustVersion ?? RUST_VERSION;
  await $`rustup toolchain install ${version}`;
  await $`rustup default ${version}`;
};

export const addBootstrapRustToolchainCommand = (command: Command) =>
  command
    .command("bootstrap-rust-toolchain")
    .description("Install only the Rust toolchain")
    .option(
      "--rust-version <version>",
      `The version of Rust to use (default: ${RUST_VERSION})`,
    )
    .action(async (options) => {
      await bootstrapRustToolchain(options);
    });
