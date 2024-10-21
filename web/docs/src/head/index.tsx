import { useRouter } from "next/router";
import { useConfig } from "nextra-theme-docs";
const Head = () => {
  const config = useConfig();
  const { route } = useRouter();

  const title = config.title + (route === "/" ? "" : " - Conformal");

  return (
    <>
      <title>{title}</title>
      {typeof config.frontMatter.description === "string" ? (
        <meta name="description" content={config.frontMatter.description} />
      ) : null}
      <meta name="msapplication-TileColor" content="#fff" />
      <meta httpEquiv="Content-Language" content="en" />
    </>
  );
};

export default Head;
