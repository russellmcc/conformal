// Various utilities for running a shell and printing arguments.

const runShell = async (args: string[], options: { cwd?: string } = {}) => {
  const proc = Bun.spawn(args, {
    stdio: ["inherit", "inherit", "inherit"],
    env: process.env,
    ...options,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
};

export const pipeShell = async (args: string[], output_path: string) => {
  const proc = Bun.spawn(args, {
    stdio: ["inherit", Bun.file(output_path), "inherit"],
    env: process.env,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")} > ${output_path}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
};

export const gatherShell = async (args: string[]): Promise<Response> => {
  const proc = Bun.spawn(args, {
    stdio: ["inherit", "pipe", "inherit"],
    env: process.env,
  });
  console.log(`$ ${args.map((x) => `"${x}"`).join(" ")}`);
  await proc.exited;
  if (proc.exitCode !== 0) {
    process.exit(proc.exitCode ?? undefined);
  }
  return new Response(proc.stdout);
};

export default runShell;
