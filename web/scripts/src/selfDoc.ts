import { command } from "./cli";
import { Help } from "@commander-js/extra-typings";

const help = new Help();

const program = command();
console.log(`# ${program.name()} Reference`);
console.log("");
console.log(help.commandDescription(program));
console.log("");
for (const c of program.commands) {
  console.log(`## \`${c.name()}\``);
  console.log("");
  console.log(c.description());
  console.log("");
  console.log(`Usage: \`bun x ${help.commandUsage(c)}\``);
  console.log("");
  for (const option of help.visibleOptions(c)) {
    if (option.name() === "help") continue;
    console.log(`### \`--${option.name()}\``);
    console.log("");
    if (option.required) {
      console.log("This option is **required**");
      console.log("");
    }
    console.log(`\`${help.optionTerm(option)}\``);
    console.log("");
    console.log(help.optionDescription(option));
    console.log("");
  }
  for (const arg of help.visibleArguments(c)) {
    console.log(`### \`${arg.name()}\``);
    console.log("");
    console.log(`\`${help.argumentTerm(arg)}\``);
    console.log("");
    console.log(help.argumentDescription(arg));
    console.log("");
  }
}
