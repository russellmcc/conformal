import { $ } from "bun";
import { describe, test } from "bun:test";

const MINUTE = 60_000;

describe("conformal documentation", () => {
  test(
    "can build",
    async () => {
      await $`bun run web-build docs`;
    },
    2 * MINUTE,
  );
});
