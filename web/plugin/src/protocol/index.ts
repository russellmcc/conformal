import { z } from "zod";
import { default as GenericTransport } from "../transport";

export const Value = z.union([
  z.number(),
  z.string(),
  z.boolean(),
  z.instanceof(Uint8Array),
]);
export type Value = z.infer<typeof Value>;

export const Request = z.union([
  z.object({
    m: z.literal("subscribe"),
    path: z.string(),
  }),
  z.object({
    m: z.literal("unsubscribe"),
    path: z.string(),
  }),
  z.object({
    m: z.literal("set"),
    path: z.string(),
    value: Value,
  }),
]);

export type Request = z.infer<typeof Request>;

export const Response = z.union([
  z.object({
    m: z.literal("values"),
    values: z.record(Value),
  }),
  z.object({
    m: z.literal("subscribe_error"),
    path: z.string(),
  }),
]);

export type Response = z.infer<typeof Response>;
export type Transport = GenericTransport<Request, Response>;