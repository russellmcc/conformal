import { DocsThemeConfig, useConfig } from "nextra-theme-docs";

const themeConfig: DocsThemeConfig = {
  project: {
    link: "https://github.com/russellmcc/conformal",
  },
  docsRepositoryBase:
    "https://github.com/russellmcc/conformal/tree/main/web/docs",
  logo: <b>Conformal</b>,
  feedback: {
    useLink: () => {
      const config = useConfig();
      return `https://github.com/russellmcc/conformal/discussions/new?category=q-a&title=Feedback regarding ${config.title}`;
    },
  },
  footer: {
    component: <></>,
  },
};

export default themeConfig;
