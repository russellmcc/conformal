import { BundleData } from "./bundleData";
import { Config } from "./configArg";
import runShell from "./runShell";
import { withDir } from "tmp-promise";
import { join, resolve } from "node:path";

const expectFile = async (path: string) => {
  if (!(await Bun.file(path).exists())) {
    console.error(`Expected ${path} to exist`);
    process.exit(1);
  }
  return path;
};

export const createBundle = async ({
  packageRoot,
  bundleData,
  config,
  dist,
  linkToLibrary,
}: {
  packageRoot: string;
  bundleData: BundleData;
  config: Config;
  dist: boolean;
  linkToLibrary: boolean;
}) => {
  const bundlePath = `target/${config}/${bundleData.name}.vst3`;

  // Make sure "dev mode" is on
  await runShell(["defaults", "write", bundleData.id, "dev_mode", "true"]);

  // Delete the bundle if it exists.
  await runShell(["rm", "-rf", bundlePath]);

  // Create the binary directory for the bundle.
  await runShell(["mkdir", "-p", `${bundlePath}/Contents/MacOS`]);

  const filename = `lib${bundleData.rustPackage}.dylib`;
  const bundleDylibPath = `${bundlePath}/Contents/MacOS/${filename}`;

  // If we are not distributing, copy the native dylib into the bundle.
  if (!dist) {
    const nativeDylibPath = await expectFile(
      `target/${config}/lib${bundleData.rustPackage}.dylib`,
    );

    await runShell(["cp", nativeDylibPath, bundleDylibPath]);
  } else {
    // Otherwise, we need to lipo the two targets together
    const armDylibPath = await expectFile(
      `target/aarch64-apple-darwin/${config}/lib${bundleData.rustPackage}.dylib`,
    );
    const x86DylibPath = await expectFile(
      `target/x86_64-apple-darwin/${config}/lib${bundleData.rustPackage}.dylib`,
    );
    await withDir(
      async ({ path: tmpDir }) => {
        const universalDylib = join(
          tmpDir,
          `lib${bundleData.rustPackage}.dylib`,
        );
        await runShell([
          "lipo",
          "-create",
          armDylibPath,
          x86DylibPath,
          "-output",
          universalDylib,
        ]);
        await runShell(["mv", universalDylib, bundleDylibPath]);
      },
      { unsafeCleanup: true },
    );
  }

  // Write the plist.
  await Bun.write(
    `${bundlePath}/Contents/Info.plist`,
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>${filename}</string>
    <key>CFBundleGetInfoString</key>
    <string>vst3</string>
    <key>CFBundleIconFile</key>
    <string></string>
    <key>CFBundleIdentifier</key>
    <string>${bundleData.id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${bundleData.name}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleVersion</key>
    <string>${bundleData.version}</string>
</dict>
</plist>`,
  );

  // Write the pkginfo.
  await Bun.write(`${bundlePath}/Contents/PkgInfo`, `BNDL${bundleData.sig}`);

  // Copy the web build into the bundle.
  await runShell(["mkdir", "-p", `${bundlePath}/Contents/Resources`]);

  await runShell([
    "cp",
    "-R",
    `${packageRoot}/dist`,
    `${bundlePath}/Contents/Resources/web-ui`,
  ]);

  // Codesign the bundle with provisional signature to allow debugging
  await runShell(["codesign", "--deep", "-s", "-", bundlePath]);

  // Link into the vst3 directory.
  const bundle_absolute_path = resolve(bundlePath);

  if (linkToLibrary) {
    const home = process.env.HOME;
    if (!home) {
      throw new Error("HOME environment variable is not set");
    }
    const bundle_dest = resolve(
      `${home}/Library/Audio/Plug-Ins/VST3/${bundleData.name}.vst3`,
    );

    await runShell(["rm", "-rf", bundle_dest]);
    await runShell(["ln", "-sf", bundle_absolute_path, bundle_dest]);
  }
};
