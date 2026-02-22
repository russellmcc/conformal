import { z } from "zod";

export const InfoSchema = z.object({
  /** The title of the parameter */
  title: z.string(),
  /** Information that depends on the type of the parameter */
  type_specific: z.union([
    z.object({
      t: z.literal("numeric"),
      /** The default value of the parameter */
      default: z.number(),
      /** The valid range of the parameter */
      valid_range: z.tuple([z.number(), z.number()]).readonly(),
      /** The units of the parameter */
      units: z.string(),
    }),
    z.object({
      t: z.literal("enum"),
      /** The default value of the parameter */
      default: z.string(),
      /** The values of the parameter */
      values: z.array(z.string()).readonly(),
    }),
    z.object({
      t: z.literal("switch"),
      /** The default value of the parameter */
      default: z.boolean(),
    }),
  ]),
});
/**
 * Information about a parameter.
 *
 * @group Types
 */
export type Info = z.infer<typeof InfoSchema>;
