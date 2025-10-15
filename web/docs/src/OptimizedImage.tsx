import ExportedImage, { ExportedImageProps } from "next-image-export-optimizer";

export type ImageProps = Omit<ExportedImageProps, "">;

const Image = (props: ImageProps) => (
  <ExportedImage {...props} basePath="/conformal" />
);

export default Image;
