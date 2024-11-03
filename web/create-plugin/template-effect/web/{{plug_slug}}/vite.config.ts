import react from "@vitejs/plugin-react-swc";
import license from "rollup-plugin-license";
import { join } from "path";

const unwrap = (x: string | null, name: string, field: string): string => {
  if (x === null) {
    throw new Error(`Malformed dependency ${name} (${field})`);
  }
  return x;
};

/** @type {import('vite').UserConfig} */
export default {
  plugins: [
    react(),
    license({
      thirdParty: {
        output: {
          file: join(__dirname, "installer_resources", "license.txt"),
          template: (dependencies) =>
            dependencies
              .map(
                (dependency) =>
                  `-----
${unwrap(dependency.name, "unknown", "name")} ${unwrap(dependency.version, dependency.name!, "version")} (${unwrap(dependency.license, dependency.name!, "license")})

${unwrap(dependency.licenseText, dependency.name!, "licenseText")}
`,
              )
              .join("\n"),
        },
        allow: {
          failOnUnlicensed: true,
          failOnViolation: true,
          test: "(MIT OR ISC)",
        },
      },
    }),
  ],
};
