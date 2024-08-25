#!/usr/bin/env bun

import { command } from "./cli";

const program = command();
await program.parseAsync();
