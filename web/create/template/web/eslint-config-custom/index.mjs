import { defineConfig } from "eslint/config";
import globals from "globals";
import js from "@eslint/js";
import ts from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import react from "eslint-plugin-react";
import preferArrowFunctions from "eslint-plugin-prefer-arrow-functions";

export default defineConfig([
  js.configs.recommended,
  {
    files: ["**/*.ts", "**/*.tsx"],
    extends: [
      ts.configs.recommended,
      ts.configs.strictTypeChecked,
      ts.configs.stylisticTypeChecked,
    ],
    rules: {
      "@typescript-eslint/no-empty-function": "off",
      "@typescript-eslint/no-unsafe-type-assertion": "error",
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/no-non-null-assertion": "off",
      "@typescript-eslint/restrict-template-expressions": [
        "error",
        { allowNumber: true },
      ],
      "@typescript-eslint/no-unnecessary-condition": [
        "error",
        { allowConstantLoopConditions: true },
      ],
      "@typescript-eslint/consistent-type-definitions": ["error", "type"],
    },
  },
  react.configs.flat.recommended ?? [],
  react.configs.flat["jsx-runtime"] ?? [],
  reactHooks.configs.flat.recommended,
  reactRefresh.configs.recommended,
  preferArrowFunctions.configs.all,
  {
    ignores: ["dist"],
  },
  {
    linterOptions: {
      reportUnusedDisableDirectives: true,
    },
    rules: {
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
      "react-hooks/exhaustive-deps": "error",
      "prefer-arrow-functions/prefer-arrow-functions": [
        "error",
        {
          returnStyle: "implicit",
        },
      ],
      "no-warning-comments": "error",
    },
    settings: {
      react: {
        version: "detect",
      },
    },
    languageOptions: {
      ecmaVersion: "latest",
      globals: {
        ...globals.browser,
        __dirname: "readonly",
      },
      parserOptions: {
        sourceType: "module",
      },
    },
  },
]);
