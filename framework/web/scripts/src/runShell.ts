// Various utilities for running a shell and printing arguments.

const runShell = async (args: readonly string[]) => {
  const proc = Bun.spawn(args as string[], {
    stdio: ["inherit", "inherit", "inherit"],
    env: process.env,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
};

export const pipeShell = async (
  args: readonly string[],
  output_path: string,
) => {
  const proc = Bun.spawn(args as string[], {
    stdio: ["inherit", Bun.file(output_path), "inherit"],
    env: process.env,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")} > ${output_path}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
};

export const gatherShell = async (args: readonly string[]) => {
  const proc = Bun.spawn(args as string[], {
    stdio: ["inherit", "pipe", "inherit"],
    env: process.env,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
  return proc.stdout;
};

export default runShell;
