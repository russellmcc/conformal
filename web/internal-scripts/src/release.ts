import { Command } from "@commander-js/extra-typings";
import { $ } from "bun";
import { deployDocs } from "./deployDocs";
import { cleanWorkspaceProtocols } from "./cleanWorkspace";
import { z } from "zod";

const crateDependencySchema = z.object({
  name: z.string(),
  source: z.union([z.string(), z.null()]),
});

const crateSchema = z.object({
  name: z.string(),
  version: z.string(),
  dependencies: z.array(crateDependencySchema),
});

const cargoMetadataSchema = z.object({
  packages: z.array(crateSchema),
});

type RawCrateInfo = z.infer<typeof crateSchema>;

type CrateInfo = {
  name: string;
  version: string;
  workspaceDependencies: string[];
};

const fromRawCrateInfo = (rawCrateInfo: RawCrateInfo): CrateInfo => ({
  name: rawCrateInfo.name,
  version: rawCrateInfo.version,
  workspaceDependencies: rawCrateInfo.dependencies
    .filter((d) => d.source === null)
    .map((d) => d.name),
});

const getCargoWorkspaceCrates = async (): Promise<CrateInfo[]> =>
  cargoMetadataSchema
    .parse(
      JSON.parse(await $`cargo metadata --format-version 1 --no-deps`.text()),
    )
    .packages.map(fromRawCrateInfo);

const getPublishedCrateVersion = async (
  crate: string,
): Promise<string | undefined> => {
  const searchResult = await $`cargo search ${crate}`.text();
  const published = searchResult
    .split("\n")
    .find((line) => line.includes(crate));
  if (!published) {
    return;
  }
  const quotedVersion = published.split(" ")[2]?.trim();
  if (!quotedVersion) {
    return;
  }

  // Use regex to check that first and last characters are quotes
  if (!/^"[^"]*"$/.test(quotedVersion)) {
    return;
  }
  return quotedVersion.slice(1, -1);
};

const publishCrate = async (crate: string) => {
  await $`cargo publish --package ${crate}`;
};

const topologicallySortCrateInfos = (crateInfos: CrateInfo[]): CrateInfo[] => {
  const sorted: CrateInfo[] = [];
  const visited = new Set<string>();
  const visit = (crate: CrateInfo) => {
    if (visited.has(crate.name)) {
      return;
    }
    visited.add(crate.name);
    for (const dependency of crate.workspaceDependencies) {
      visit(crateInfos.find((c) => c.name === dependency)!);
    }
    sorted.push(crate);
  };
  for (const crate of crateInfos) {
    visit(crate);
  }
  return sorted;
};

export const release = async ({ skipPublish }: { skipPublish?: boolean }) => {
  skipPublish ??= false;

  if (skipPublish) {
    return;
  }

  // Publish cargo packages in topological order, skipping packages that are already published
  for (const crate of topologicallySortCrateInfos(
    await getCargoWorkspaceCrates(),
  )) {
    const publishedVersion = await getPublishedCrateVersion(crate.name);
    if (publishedVersion === crate.version) {
      continue;
    }
    console.log(
      `Publishing ${crate.name} ${crate.version} ${publishedVersion}`,
    );
    try {
      await publishCrate(crate.name);
    } catch (error) {
      console.error(error);
    }
  }

  await cleanWorkspaceProtocols();

  // Publish npm packages
  // Note that there is now a native `bun publish`, but we can't use it for a few reasons:
  //  - https://github.com/oven-sh/bun/issues/5050 <- bun publish doesn't support monorepos (we could
  //    simply work around this by sorting packages topologically and publishing each one)
  //  - https://github.com/oven-sh/bun/issues/15601 <- bun publish doesn't support `provenance` flag
  await $`bunx @morlay/bunpublish --provenance`;

  // Publish documentation
  await deployDocs();
};

export const addReleaseCommand = (command: Command) => {
  command
    .command("release")
    .option("--skip-publish", "Skip publishing to npm and cargo")
    .action(async (opts) => {
      await release(opts);
    });
};
