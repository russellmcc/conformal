import { z } from "zod";

export const TypeSpecific = z.union([
  z.object({
    t: z.literal("numeric"),
    default: z.number(),
    valid_range: z.tuple([z.number(), z.number()]).readonly(),
    units: z.string(),
  }),
  z.object({
    t: z.literal("enum"),
    default: z.string(),
    values: z.array(z.string()).readonly(),
  }),
  z.object({
    t: z.literal("switch"),
    default: z.boolean(),
  }),
]);
export type TypeSpecific = z.infer<typeof TypeSpecific>;

export const Info = z.object({
  title: z.string(),
  type_specific: TypeSpecific,
});
export type Info = z.infer<typeof Info>;
