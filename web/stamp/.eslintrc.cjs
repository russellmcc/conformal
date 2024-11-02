module.exports = {
  root: true,
  extends: ["eslint-config-custom"],
  parserOptions: {
    project: ["./tsconfig.json"],
    tsconfigRootDir: __dirname,
  },
};
