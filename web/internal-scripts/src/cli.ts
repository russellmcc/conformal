import { Command } from "@commander-js/extra-typings";
import { addReleaseCommand } from "./release";
import { addPostpackCommand } from "./postpack";
import { addPrepackCommand } from "./prepack";
import { addDeployDocsCommand } from "./deployDocs";
import {
  addRustChangeCommand,
  addRustPrepareReleaseCommand,
} from "./rustChange";

export const command = () => {
  const command = new Command("conformal-internal-scripts").description(
    "This is a CLI entry point for various scripts needed to build the conformal audio framework.",
  );

  addReleaseCommand(command);
  addPrepackCommand(command);
  addPostpackCommand(command);
  addDeployDocsCommand(command);
  addRustChangeCommand(command);
  addRustPrepareReleaseCommand(command);

  return command;
};
