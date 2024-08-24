export const indexOf = <T>(v: T, vs: T[]): number | undefined => {
  const ret = vs.indexOf(v);
  if (ret === -1) {
    return undefined;
  }
  return ret;
};

export const clamp = (v: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, v));
