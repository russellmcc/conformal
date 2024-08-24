import { CommandLineAction } from "@rushstack/ts-command-line";
import { $ } from "bun";
import { failUnless } from "./util";

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

export const checkTodos = async (): Promise<boolean> =>
  // Returns false if there are any files with '*.rs' in the name that contain the string 'TODO'
  (
    await $`grep TODO ${await Array.fromAsync(
      filter($`git ls-files`.lines(), (f) => f.includes(".rs")),
    )}`.nothrow()
  ).exitCode != 0;

export const execute = async () => {
  failUnless(await checkTodos());
};

export class CheckTodosAction extends CommandLineAction {
  public constructor() {
    super({
      actionName: "check-todo",
      summary: "Check if any rust files contain TODOs",
      documentation: "",
    });
  }

  public async onExecute(): Promise<void> {
    await execute();
  }
}
