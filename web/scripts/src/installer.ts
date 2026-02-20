import { BundleData } from "./bundleData";
import { withDir } from "tmp-promise";
import runShell, { gatherShell, pipeShell } from "./runShell";
import { basename, join, dirname } from "path";
import { env } from "process";
import { z } from "zod";

// Get the path to the rust package.
const getRustPackagePath = async (rustPackage: string) => {
  const metadataParser = z.object({
    packages: z.array(
      z.object({ name: z.string(), manifest_path: z.string() }),
    ),
  });
  const maybeMetadata = await metadataParser.safeParseAsync(
    await (
      await gatherShell([
        "cargo",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
      ])
    ).json(),
  );
  if (!maybeMetadata.success) {
    throw new Error(maybeMetadata.error.message);
  }
  for (const p of maybeMetadata.data.packages) {
    if (p.name === rustPackage) {
      return dirname(p.manifest_path);
    }
  }
  throw new Error(`Could not find package ${rustPackage}`);
};

/**
 * Creates an installer.  Must be called from the workspace root!
 *
 */
export const createInstaller = async ({
  packageRoot,
  bundleData,
}: {
  packageRoot: string;
  bundleData: BundleData;
}) => {
  const rustPackagePath = await getRustPackagePath(bundleData.rustPackage);
  const bundlePath = `target/release/${bundleData.name}.vst3`;

  const notaryToolKeychainPath = env.NOTARYTOOL_KEYCHAIN_PATH;

  // Check required env variables ahead of time
  const developerIdApplication = env.DEVELOPER_ID_APPLICATION;
  if (!developerIdApplication) {
    throw new Error(
      "No application Developer ID set, make sure `DEVELOPER_ID_APPLICATION` is set",
    );
  }
  const developerIdInstaller = env.DEVELOPER_ID_INSTALLER;
  if (!developerIdInstaller) {
    throw new Error(
      "No installater Developer ID set, make sure `DEVELOPER_ID_INSTALLER` is set",
    );
  }
  const notarytoolCredentialsKeychainItem =
    env.NOTARYTOOL_CREDENTIALS_KEYCHAIN_ITEM;
  if (!notarytoolCredentialsKeychainItem) {
    throw new Error(
      `No notary tool credentials keychain item set, make sure \`NOTARYTOOL_CREDENTIALS_KEYCHAIN_ITEM\` is set. 

To get these credentials, run something like this:

\`\`\`
% xcrun notarytool store-credentials "notarytool-password"
               --apple-id "<AppleID>"
               --team-id <DeveloperTeamID>
               --password <app-specific password>
\`\`\`

This will create a keychain item named notarytool-password

More info [here](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution/customizing_the_notarization_workflow)
`,
    );
  }

  type ManifestEntry = {
    local: string;
    dest: string;
    sign: boolean;
  };

  type InternalPkgData = {
    ident: string;
    path: string;
  };

  const makeInternalPkg = async (
    items: ManifestEntry[],
    ident: string,
    outputDir: string,
  ): Promise<InternalPkgData> => {
    const data = { current: null as InternalPkgData | null };
    // First make a temp directory to arrange our manifests
    await withDir(
      async ({ path: tmpDir }) => {
        // Copy each manifest entry into place in the temp directory
        for (const { local, dest, sign } of items) {
          const itemDest = `${tmpDir}/${dest}/${basename(local)}`;
          await runShell(["ditto", local, itemDest]);
          if (sign) {
            await runShell([
              "codesign",
              "-f",
              "--options=runtime",
              "--timestamp",
              "-s",
              developerIdApplication,
              itemDest,
            ]);

            // Check bundle signature
            await runShell([
              "codesign",
              "-vvv",
              "--deep",
              "--strict",
              itemDest,
            ]);

            // Print extra signature info (useful for debugging)
            await runShell([
              "codesign",
              "-dvv",
              "--deep",
              "--strict",
              itemDest,
            ]);
          }
        }
        const qualifiedIdent = `${bundleData.id}.${ident}`;
        const relativePath = `${ident}.pkg`;
        const path = join(outputDir, relativePath);
        // Create the pkg file
        await runShell([
          "pkgbuild",
          "--root",
          tmpDir,
          "--identifier",
          qualifiedIdent,
          "--version",
          bundleData.version,
          "--sign",
          developerIdInstaller,
          path,
        ]);

        // check signature
        await runShell(["pkgutil", "--check-signature", path]);

        data.current = { ident: qualifiedIdent, path: relativePath };
      },
      { unsafeCleanup: true },
    );
    return data.current!;
  };

  await withDir(
    async ({ path: tmpDir }) => {
      const pkgDir = join(tmpDir, "pkgs");

      await runShell(["mkdir", "-p", pkgDir]);
      const { ident: vstIdent, path: vstPkgPath } = await makeInternalPkg(
        [
          {
            local: bundlePath,
            dest: "Library/Audio/Plug-Ins/VST3",
            sign: true,
          },
        ],
        "pkg.vst3",
        pkgDir,
      );

      // Write the distribution xml
      const distributionPath = join(pkgDir, "distribution.xml");
      await Bun.write(
        distributionPath,
        `<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="1">
  <title>${bundleData.name} ${bundleData.version}</title>
  <license file="license.txt"/>
  <pkg-ref id="${vstIdent}"/>
  <options require-scripts="false" customize="never" rootVolumeOnly="true"/>
  <domains enable_localSystem="true" enable_currentUserHome="true"/>
  <choices-outline>
    <line choice="${vstIdent}"/>
  </choices-outline>
  <choice id="${vstIdent}" title="VST3">
    <pkg-ref id="${vstIdent}"/>
  </choice>
  <pkg-ref id="${vstIdent}" version="${bundleData.version}" onConclusion="none">${vstPkgPath}</pkg-ref>
</installer-gui-script>`,
      );

      const dmgRoot = join(tmpDir, "dmgRoot");
      await runShell(["mkdir", "-p", dmgRoot]);

      const pkgPath = join(dmgRoot, `Install ${bundleData.name}.pkg`);
      const rsrcPath = join(tmpDir, "installer_resources");
      await runShell(["mkdir", "-p", rsrcPath]);
      const licenseTmpPath = join(tmpDir, "license_temp");
      await pipeShell(
        [
          "cargo",
          "about",
          "generate",
          "-m",
          `${rustPackagePath}/Cargo.toml`,
          `${rustPackagePath}/about.hbs`,
        ],
        licenseTmpPath,
      );
      await pipeShell(
        [
          "cat",
          licenseTmpPath,
          join(packageRoot, "installer_resources", "license.txt"),
        ],
        join(rsrcPath, "license.txt"),
      );

      await runShell([
        "productbuild",
        "--distribution",
        distributionPath,
        "--package-path",
        pkgDir,
        "--resources",
        rsrcPath,
        "--version",
        bundleData.version,
        "--sign",
        developerIdInstaller,
        pkgPath,
      ]);

      // check signature
      await runShell(["pkgutil", "--check-signature", pkgPath]);

      // Now make a dmg, deleting any previous ones
      const dmgName = `${bundleData.name}.dmg`;
      const dmgOutput = `target/release/${dmgName}`;
      const dmgInitialPath = join(tmpDir, "temp.dmg");
      const dmgTmpPath = join(tmpDir, dmgName);
      await runShell([
        "hdiutil",
        "create",
        dmgInitialPath,
        "-ov",
        "-volname",
        bundleData.name,
        "-fs",
        "HFS+",
        "-srcfolder",
        dmgRoot,
      ]);
      await runShell([
        "hdiutil",
        "convert",
        dmgInitialPath,
        "-format",
        "UDZO",
        "-o",
        dmgTmpPath,
      ]);

      // Notarize!
      await runShell(
        [
          "xcrun",
          "notarytool",
          "submit",
          dmgTmpPath,
          "--keychain-profile",
          notarytoolCredentialsKeychainItem,
          "--wait",
        ].concat(
          notaryToolKeychainPath ? ["--keychain", notaryToolKeychainPath] : [],
        ),
      );

      // Staple!
      await runShell(["xcrun", "stapler", "staple", dmgTmpPath]);

      await runShell(["rm", "-f", dmgOutput]);
      await runShell(["mv", dmgTmpPath, dmgOutput]);
    },
    { unsafeCleanup: true },
  );
};
