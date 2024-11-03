#!/usr/bin/env bun

import { Command } from "@commander-js/extra-typings";
import { Config, metadatas, postBuild } from "./config";
import { toEnv } from "@conformal/create-plugin";
import { buildStampCommand } from "@conformal/stamp";
import path from "node:path";

const command = buildStampCommand<keyof Config>({
  command: new Command(),
  metadatas,
  toEnv,
  toDest: (config) => Promise.resolve(config.proj_slug),
  toTemplate: () =>
    Promise.resolve(
      path.join(path.dirname(import.meta.path), "..", "template"),
    ),
  postBuild,
});

await command.parseAsync();
