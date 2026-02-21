/* eslint-env node */
import { Footer, Layout, Navbar } from "nextra-theme-docs";
import { Anchor, Head } from "nextra/components";
import { getPageMap } from "nextra/page-map";
import "nextra-theme-docs/style.css";
import React from "react";
import { Metadata } from "next";

export const metadata: Metadata = {
  metadataBase: new URL("https://github.com/russellmcc/conformal"),
  description: "Conformal Audio Framework",
  applicationName: "Conformal Audio Framework",
  generator: "Next.js",
  appleWebApp: {
    title: "Conformal Audio Framework",
  },
  other: {
    "msapplication-TileColor": "#fff",
  },
};

const MyLayout = async ({ children }: { children: React.ReactNode }) => {
  const navbar = (
    <Navbar
      logo={<b>Conformal</b>}
      projectLink="https://github.com/russellmcc/conformal"
    />
  );
  return (
    <html lang="en" suppressHydrationWarning>
      <Head />
      <body>
        <Layout
          navbar={navbar}
          copyPageButton={false}
          footer={<Footer />}
          editLink="Edit this page"
          docsRepositoryBase="https://github.com/russellmcc/conformal/tree/main/web/docs"
          feedback={{
            content: null,
          }}
          toc={{
            extraContent: (
              <Anchor
                href="https://github.com/russellmcc/conformal/discussions/new?category=q-a&title=Feedback on docs"
                // This is copy-pasted from the docs theme.
                className="x:text-xs x:font-medium x:transition x:text-gray-600 x:dark:text-gray-400 x:hover:text-gray-800 x:dark:hover:text-gray-200 x:contrast-more:text-gray-700 x:contrast-more:dark:text-gray-100"
              >
                Question? Give us feedback
              </Anchor>
            ),
          }}
          sidebar={{ defaultMenuCollapseLevel: 1 }}
          pageMap={await getPageMap()}
        >
          {children}
        </Layout>
      </body>
    </html>
  );
};

export default MyLayout;
