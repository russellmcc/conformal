const withNextra = require("nextra")({
  theme: "nextra-theme-docs",
  themeConfig: "./theme.config.tsx",
  latex: true,
});

module.exports = withNextra({
  output: "export",
  trailingSlash: true,
  basePath: "/conformal",
  images: {
    unoptimized: true,
  },
});
