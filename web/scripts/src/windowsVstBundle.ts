import { BundleData } from "./bundleData";
import { Config } from "./configArg";
import { join, resolve } from "node:path";
import { rm, mkdir, copyFile, cp, symlink } from "node:fs/promises";
import { rcedit } from "rcedit";

const expectFile = async (path: string) => {
  if (!(await Bun.file(path).exists())) {
    console.error(`Expected ${path} to exist`);
    process.exit(1);
  }
  return path;
};

/**
 * Creates a Steinberg VST3 bundle for Windows.
 *
 * Produces the following structure:
 *
 * ```
 * <Name>.vst3/
 *   Contents/
 *     Resources/
 *       web-ui/
 *     x86_64-win/
 *       <Name>.vst3   <- the DLL
 * ```
 */
export const createWindowsVstBundle = async ({
  packageRoot,
  bundleData,
  config,
  linkToLibrary,
}: {
  packageRoot: string;
  bundleData: BundleData;
  config: Config;
  linkToLibrary: boolean;
}) => {
  const bundlePath = `target/${config}/${bundleData.name}.vst3`;

  await rm(bundlePath, { recursive: true, force: true });

  const targetDir = join(bundlePath, "Contents", "x86_64-win");
  const resourcesDir = join(bundlePath, "Contents", "Resources");
  await mkdir(targetDir, { recursive: true });
  await mkdir(resourcesDir, { recursive: true });

  const dllPath = await expectFile(
    `target/${config}/${bundleData.rustPackage}.dll`,
  );
  const dllDestPath = join(targetDir, `${bundleData.name}.vst3`);
  await copyFile(dllPath, dllDestPath);

  await rcedit(dllDestPath, {
    "version-string": {
      CompanyName: bundleData.vendor,
      // rcedit's types call this InternalFilename, but the actual
      // VS_VERSION_INFO field name is InternalName.
      InternalFilename: bundleData.id,
      ProductName: bundleData.name,
    },
    "product-version": bundleData.version,
    "file-version": bundleData.version,
  });

  await cp(join(packageRoot, "dist"), join(resourcesDir, "web-ui"), {
    recursive: true,
  });

  if (linkToLibrary) {
    const localAppData = process.env.LOCALAPPDATA;
    if (!localAppData) {
      throw new Error("LOCALAPPDATA environment variable is not set");
    }
    const vst3Dir = join(localAppData, "Programs", "Common", "VST3");
    await mkdir(vst3Dir, { recursive: true });
    const bundleDest = join(vst3Dir, `${bundleData.name}.vst3`);
    await rm(bundleDest, { recursive: true, force: true });
    await symlink(resolve(bundlePath), bundleDest, "junction");
  }
};
