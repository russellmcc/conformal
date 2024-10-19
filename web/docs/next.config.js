import nextra from "nextra";

const withNextra = nextra({
  theme: "nextra-theme-docs",
  themeConfig: "./theme.config.tsx",
  latex: true,
});

export default withNextra({
  output: "export",
  trailingSlash: true,
  basePath: "/conformal",
  images: {
    unoptimized: true,
  },
});
