import { defineConfig } from "eslint/config";
import config from "eslint-config-custom";
import { FlatCompat } from "@eslint/eslintrc";

const compat = new FlatCompat({
  baseDirectory: import.meta.dirname,
});

export default defineConfig([
  config,
  compat.extends("plugin:@next/next/recommended"),
  {
    ignores: ["out", ".next"],
  },
  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      parserOptions: {
        project: ["./tsconfig.json"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "react-refresh/only-export-components": "off",
      // Some of the blog posts do ref crimes, disable for now.
      "react-hooks/refs": "off",
      "@typescript-eslint/triple-slash-reference": "off",
      // We use a custom loader for next images to statically optimize them,
      // but we load un-optimized SVGs with img tags.
      "@next/next/no-img-element": "off",
    },
  },
]);
