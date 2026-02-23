import { BundleData } from "./bundleData";
import { withDir } from "tmp-promise";
import runShell, { gatherShell, pipeShell } from "./runShell";
import { join, dirname, resolve } from "path";
import { createHash } from "crypto";
import { z } from "zod";
import { readFile } from "node:fs/promises";

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
 * Derives a deterministic GUID from a string using SHA-256.
 *
 * MSI requires a stable UpgradeCode GUID that stays the same across versions
 * so Windows can detect upgrades. We derive this from the bundle ID so
 * developers don't need to manage it manually.
 */
const deriveGuid = (input: string): string => {
  const hash = createHash("sha256").update(input).digest("hex");
  return [
    hash.slice(0, 8),
    hash.slice(8, 12),
    hash.slice(12, 16),
    hash.slice(16, 20),
    hash.slice(20, 32),
  ]
    .join("-")
    .toUpperCase();
};

const textToRtf = (text: string): string => {
  const escaped = text
    .replace(/\\/g, "\\\\")
    .replace(/\{/g, "\\{")
    .replace(/\}/g, "\\}")
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .replace(/\n/g, "\\par\n")
    .replace(/[\u0080-\uFFFF]/g, (ch) => {
      const code = ch.codePointAt(0)!;
      const signed = code > 32767 ? code - 65536 : code;
      return `\\u${signed}?`;
    });
  return `{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fswiss Helvetica;}}\\f0\\fs20\n${escaped}\n}`;
};

const WEBVIEW2_BOOTSTRAPPER_URL =
  "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

/**
 * Generates the main WiX source file for the MSI installer.
 *
 * The harvested fragment from heat.exe is compiled separately and linked in;
 * it references VST3_INSTALL_DIR which is defined here.
 *
 * The WebView2 bootstrapper is embedded as a Binary and run via a deferred
 * Custom Action if WebView2 is not already installed. The registry key
 * checked is Microsoft's documented detection mechanism.
 */
const generateWxs = (
  bundleData: BundleData,
  upgradeCode: string,
): string => `<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product
    Id="*"
    Name="${bundleData.name}"
    Language="1033"
    Version="${bundleData.version}"
    Manufacturer="${bundleData.vendor}"
    UpgradeCode="${upgradeCode}">

    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" Platform="x64" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of [ProductName] is already installed." />
    <MediaTemplate EmbedCab="yes" />

    <Property Id="WVRTINSTALLED">
      <RegistrySearch Id="WVRTInstalled" Root="HKLM" Key="SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" Name="EBWebView" Type="raw" Win64="no" />
    </Property>

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="CommonFilesDir" Name="Common Files">
          <Directory Id="VST3Dir" Name="VST3">
            <Directory Id="VST3_INSTALL_DIR" Name="${bundleData.name}.vst3" />
          </Directory>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="${bundleData.name}" Level="1">
      <ComponentGroupRef Id="BundleFiles" />
    </Feature>

    <Binary Id="MicrosoftEdgeWebview2Setup.exe" SourceFile="MicrosoftEdgeWebview2Setup.exe" />
    <CustomAction Id="InstallWebView2Runtime" BinaryKey="MicrosoftEdgeWebview2Setup.exe" Execute="deferred" ExeCommand="/silent /install" Return="check" Impersonate="no" />
    <InstallExecuteSequence>
      <Custom Action="InstallWebView2Runtime" Before="InstallFinalize"><![CDATA[NOT(REMOVE OR WVRTINSTALLED)]]></Custom>
    </InstallExecuteSequence>

    <UIRef Id="WixUI_Minimal" />
  </Product>
</Wix>`;

/**
 * Creates a Windows MSI installer. Must be called from the workspace root.
 */
export const createWindowsInstaller = async ({
  packageRoot,
  bundleData,
}: {
  packageRoot: string;
  bundleData: BundleData;
}) => {
  const rustPackagePath = await getRustPackagePath(bundleData.rustPackage);
  const bundlePath = resolve(`target/release/${bundleData.name}.vst3`);
  const msiOutput = `target/release/${bundleData.name}.msi`;

  const upgradeCode = deriveGuid(`${bundleData.id}.msi.upgrade-code`);

  await withDir(
    async ({ path: tmpDir }) => {
      const licenseTmpPath = join(tmpDir, "license_temp");
      // Make sure we have all dependencies downloaded.
      await runShell(["cargo", "fetch"]);
      // Generate the about file. Using frozen mode will prevent us from querying
      // any flaky APIs such as clearlydefined.
      await runShell([
        "cargo",
        "about",
        "generate",
        "--frozen",
        "-m",
        `${rustPackagePath}/Cargo.toml`,
        `${rustPackagePath}/about.hbs`,
        "-o",
        licenseTmpPath,
      ]);
      const combinedLicensePath = join(tmpDir, "license_combined.txt");
      await pipeShell(
        [
          "cmd",
          "/c",
          "type",
          licenseTmpPath,
          join(packageRoot, "installer_resources", "license.txt"),
        ],
        combinedLicensePath,
      );

      const licenseText = await readFile(combinedLicensePath, "utf-8");
      const rtfPath = join(tmpDir, "license.rtf");
      await Bun.write(rtfPath, textToRtf(licenseText));

      const bootstrapperPath = join(tmpDir, "MicrosoftEdgeWebview2Setup.exe");
      const bootstrapperResponse = await fetch(WEBVIEW2_BOOTSTRAPPER_URL);
      if (!bootstrapperResponse.ok) {
        throw new Error(
          `Failed to download WebView2 bootstrapper: ${bootstrapperResponse.status}`,
        );
      }
      await Bun.write(bootstrapperPath, bootstrapperResponse);

      const wxsPath = join(tmpDir, "main.wxs");
      await Bun.write(wxsPath, generateWxs(bundleData, upgradeCode));

      const fragmentPath = join(tmpDir, "fragment.wxs");
      await runShell([
        "heat.exe",
        "dir",
        bundlePath,
        "-cg",
        "BundleFiles",
        "-gg",
        "-scom",
        "-sreg",
        "-sfrag",
        "-srd",
        "-dr",
        "VST3_INSTALL_DIR",
        "-var",
        "var.BundleDir",
        "-out",
        fragmentPath,
      ]);

      await runShell([
        "candle.exe",
        `-dBundleDir=${bundlePath}`,
        "-arch",
        "x64",
        "-ext",
        "WixUIExtension",
        "-out",
        `${tmpDir}\\`,
        wxsPath,
        fragmentPath,
      ]);

      const msiTmpPath = join(tmpDir, `${bundleData.name}.msi`);
      await runShell([
        "light.exe",
        "-ext",
        "WixUIExtension",
        `-dWixUILicenseRtf=${rtfPath}`,
        "-b",
        bundlePath,
        "-b",
        tmpDir,
        "-out",
        msiTmpPath,
        join(tmpDir, "main.wixobj"),
        join(tmpDir, "fragment.wixobj"),
      ]);

      await Bun.write(msiOutput, Bun.file(msiTmpPath));
    },
    { unsafeCleanup: true },
  );
};
