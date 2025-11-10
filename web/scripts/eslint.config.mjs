import { defineConfig } from "eslint/config";
import config from "eslint-config-custom";

export default defineConfig([
  config,
  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      parserOptions: {
        project: ["./tsconfig.json"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
]);
