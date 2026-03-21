import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { useMDXComponents as mdxComponents } from "nextra-theme-docs";
import { generateStaticParamsFor, importPage } from "nextra/pages";
import { cache } from "react";

export const generateStaticParams = generateStaticParamsFor("mdxPath");

const execFileAsync = promisify(execFile);

const getFirstPublishedTimestamp = cache(async (filePath: string) => {
  try {
    const { stdout } = await execFileAsync("git", [
      "log",
      "--follow",
      "--diff-filter=A",
      "--format=%ct",
      "--",
      filePath,
    ]);
    const [firstPublished] = stdout.trim().split("\n");
    if (!firstPublished) {
      return undefined;
    }
    const timestamp = Number.parseInt(firstPublished, 10);
    if (Number.isNaN(timestamp)) {
      return undefined;
    }
    return timestamp * 1000;
  } catch {
    return undefined;
  }
});

const formatDate = (date: Date) =>
  date.toLocaleDateString("en", {
    day: "numeric",
    month: "long",
    year: "numeric",
  });

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
  const path = params.mdxPath ?? [];
  const data = await importPage(params.mdxPath);
  const { default: MDXContent, toc, metadata, sourceCode } = data;
  const lastUpdatedDate =
    path.length > 0 && metadata.timestamp !== undefined
      ? new Date(metadata.timestamp)
      : undefined;
  const firstPublishedTimestamp =
    lastUpdatedDate === undefined
      ? undefined
      : await getFirstPublishedTimestamp(metadata.filePath);
  const firstPublishedDate =
    firstPublishedTimestamp === undefined
      ? undefined
      : new Date(firstPublishedTimestamp);
  const wrapperMetadata =
    lastUpdatedDate === undefined
      ? metadata
      : { ...metadata, timestamp: undefined };
  // seems to be a but in the linter
  // eslint-disable-next-line @typescript-eslint/unbound-method
  const Wrapper = mdxComponents().wrapper;

  return (
    <Wrapper toc={toc} metadata={wrapperMetadata} sourceCode={sourceCode}>
      <MDXContent {...props} params={params} />
      {lastUpdatedDate ? (
        <div className="x:mt-12 x:mb-8 x:text-xs x:text-gray-600 x:text-end x:dark:text-gray-400">
          {firstPublishedDate ? (
            <div>
              First published on{" "}
              <time dateTime={firstPublishedDate.toISOString()}>
                {formatDate(firstPublishedDate)}
              </time>
            </div>
          ) : null}
          <div>
            Last updated on{" "}
            <time dateTime={lastUpdatedDate.toISOString()}>
              {formatDate(lastUpdatedDate)}
            </time>
          </div>
        </div>
      ) : null}
    </Wrapper>
  );
};

export default Page;
