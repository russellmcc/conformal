import { DocsThemeConfig, useConfig } from "nextra-theme-docs";
import Head from "./src/head";
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
      const title = config.title;

      return `https://github.com/russellmcc/conformal/discussions/new?category=q-a&title=Feedback regarding ${title}`;
    },
  },
  head: Head,
  footer: {
    component: <></>,
  },
};

export default themeConfig;
