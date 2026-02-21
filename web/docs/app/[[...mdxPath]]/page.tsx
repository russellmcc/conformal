import { useMDXComponents as mdxComponents } from "nextra-theme-docs";
import { generateStaticParamsFor, importPage } from "nextra/pages";

export const generateStaticParams = generateStaticParamsFor("mdxPath");

type PageParams = {
  mdxPath?: string[];
};

type PageProps = {
  params: Promise<PageParams>;
};

export const generateMetadata = async (props: PageProps) => {
  const { mdxPath } = await props.params;
  const { metadata } = await importPage(mdxPath);
  const path = mdxPath ?? [];
  // If this isn't the root, add Conformal as a suffix.
  if (path.length > 0) {
    metadata.title = `${metadata.title} - Conformal`;
  }
  return metadata;
};

const Page = async (props: PageProps) => {
  const params = await props.params;
  const data = await importPage(params.mdxPath);
  const { default: MDXContent, toc, metadata, sourceCode } = data;
  // seems to be a but in the linter
  // eslint-disable-next-line @typescript-eslint/unbound-method
  const Wrapper = mdxComponents().wrapper;

  return (
    <Wrapper toc={toc} metadata={metadata} sourceCode={sourceCode}>
      <MDXContent {...props} params={params} />
    </Wrapper>
  );
};

export default Page;
