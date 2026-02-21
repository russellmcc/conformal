import { useMDXComponents as getDocsMDXComponents } from "nextra-theme-docs";
import { MDXComponents } from "nextra/mdx-components";

const docsComponents = getDocsMDXComponents();

export const useMDXComponents = (components: MDXComponents): MDXComponents => ({
  ...docsComponents,
  ...components,
});
