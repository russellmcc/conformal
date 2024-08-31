import { Command } from "@commander-js/extra-typings";
import { addReleaseCommand } from "./release";

export const command = () => {
  const command = new Command("conformal-internal-scripts").description(
    "This is a CLI entry point for various scripts needed to build the conformal audio framework.",
  );

  addReleaseCommand(command);

  return command;
};
