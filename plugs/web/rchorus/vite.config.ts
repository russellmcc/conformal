import react from "@vitejs/plugin-react-swc";
import license from "rollup-plugin-license";
import { join } from "path";

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
${dependency.name} ${dependency.version} (${dependency.license})

${dependency.licenseText}
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
