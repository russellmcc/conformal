import { z } from "zod";
import * as path from "path";

const BundleDataParser = z.object({
  rustPackage: z.string(),
  name: z.string(),
  vendor: z.string(),
  id: z.string(),
  sig: z.string(),
  version: z.string(),
});

export type BundleData = z.infer<typeof BundleDataParser>;

export const getBundleData = async (
  packageRoot: string,
): Promise<BundleData> => {
  const result = await BundleDataParser.safeParseAsync(
    (await Bun.file(path.join(packageRoot, "bundle.json")).json()) as unknown,
  );
  if (result.success) {
    return result.data;
  } else {
    throw new Error(result.error.message);
  }
};

export default getBundleData;
