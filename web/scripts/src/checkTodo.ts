import { $ } from "bun";
import { failUnless } from "./util";
import { Command } from "@commander-js/extra-typings";

const filter = async function* <T>(
  it: AsyncIterable<T>,
  filter: (s: T) => boolean,
): AsyncIterable<T> {
  for await (const s of it) {
    if (filter(s)) {
      yield s;
    }
  }
};

export const checkTodos = async (): Promise<boolean> => {
  // Fail early if we aren't in a git repo
  try {
    await $`git rev-parse --is-inside-work-tree`.quiet();
  } catch {
    console.error("Not in a git repo");
    return false;
  }

  const relevantFiles = await Array.fromAsync(
    filter(
      $`git ls-files -oc`.lines(),
      (f) => f.includes(".rs") || f.includes(".hbs"),
    ),
  );

  if (relevantFiles.length === 0) {
    console.error("No relevant files found in TODO checker");
    return false;
  }

  // Returns false if there are any files with '*.rs' in the name that contain the string 'TODO'
  return (await $`grep TODO ${relevantFiles}`.nothrow()).exitCode != 0;
};

export const execute = async () => {
  failUnless(await checkTodos());
};

export const addCheckTodoCommand = (command: Command) =>
  command
    .command("check-todo")
    .description("Check if any rust files contain TODOs")
    .action(async () => {
      await execute();
    });
