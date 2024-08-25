import { addCheckLfsCommand } from "./checkLfs";
import { addCheckTodoCommand } from "./checkTodo";
import { addCheckFormatCommand } from "./checkFormat";
import { addFormatCommand } from "./format";
import { addBootstrapCommand } from "./bootstrap";
import { Command } from "@commander-js/extra-typings";
import { addPackageCommand } from "./package";
import { addValidateCommand } from "./validate";
import { addCargoCommand } from "./cargo";
import { addCICommand } from "./ci";
import { addWebScriptCommand } from "./webScript";

export const command = () => {
  const command = new Command("scripts").description(
    "This is a CLI entry point for various build-related scripts.",
  );

  addBootstrapCommand(command);
  addCheckLfsCommand(command);
  addCheckTodoCommand(command);
  addCheckFormatCommand(command);
  addFormatCommand(command);
  addPackageCommand(command);
  addValidateCommand(command);
  addCargoCommand(command);
  addCICommand(command);
  addWebScriptCommand(command);
  return command;
};
