module.exports = {
    root: true,
    extends: ["eslint-config-custom", "plugin:storybook/recommended"],
    parserOptions: {
      project: ["./tsconfig.json"],
      tsconfigRootDir: __dirname,
    },
  };
  