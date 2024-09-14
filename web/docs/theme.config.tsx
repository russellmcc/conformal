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

      // Not sure what's going on here, but typescript can't seem to correctly
      // infer the type of `config` :'(.
      // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
      const title: string = config.title;

      return `https://github.com/russellmcc/conformal/discussions/new?category=q-a&title=Feedback regarding ${title}`;
    },
  },
  footer: {
    component: <></>,
  },
};

export default themeConfig;
