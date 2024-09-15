// Disable some lints since we don't have a type for package.json
/* eslint-disable @typescript-eslint/no-unsafe-member-access */
/* eslint-disable @typescript-eslint/no-unsafe-assignment */

import { Command } from "@commander-js/extra-typings";
import dts from "bun-plugin-dts";
import * as fs from "node:fs/promises";

export const prepack = async () => {
  await Bun.build({
    entrypoints: ["./src/index.ts"],
    outdir: "./dist",
    plugins: [dts()],
    target: "browser",
    packages: "external",
  });

  // Back up the package.json file using node promises api
  await fs.copyFile("./package.json", "./package.json.bak");

  // Modify the package.json's "exports" field, to use the dist directory
  const packageJson = await Bun.file("./package.json").json();
  packageJson.exports = {
    ".": {
      types: "./dist/index.d.ts",
      import: "./dist/index.js",
    },
  };

  // Write the modified package.json back to the file system
  await Bun.write("./package.json", JSON.stringify(packageJson, null, 2));
};

export const addPrepackCommand = (command: Command) => {
  command
    .command("ts-browser-prepack")
    .description("standard prepack script for ts browser libs")
    .action(async () => {
      await prepack();
    });
};
