import { CommandLineParser } from "@rushstack/ts-command-line";
import { CheckLFSAction } from "./checkLfs";
import { CheckTodosAction } from "./checkTodo";
import { CheckFormatAction } from "./checkFormat";
import { FormatAction } from "./format";
import { WebScriptAction } from "./webScript";
import { CargoAction } from "./cargo";
import { PackageAction } from "./package";
import { ValidateAction } from "./validate";
import { CIAction } from "./ci";

export class CommandLine extends CommandLineParser {
  public constructor() {
    super({
      toolFilename: "scripts",
      toolDescription: `This is a framework for building plug-ins in typescript and rust!

This package is a CLI entry point for various build scripts.`,
    });

    this.addAction(new CheckLFSAction());
    this.addAction(new CheckTodosAction());
    this.addAction(new CheckFormatAction());
    this.addAction(new FormatAction());
    this.addAction(new WebScriptAction());
    this.addAction(new CargoAction());
    this.addAction(new PackageAction());
    this.addAction(new ValidateAction());
    this.addAction(new CIAction());
  }
}
