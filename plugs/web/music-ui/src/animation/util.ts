export const fade = (
  x: number,
  forward: boolean,
  elapsed: number,
  duration: number,
): number => {
  const delta = ((forward ? 1 : -1) * elapsed) / duration;
  return Math.min(1, Math.max(0, x + delta));
};

// Simple quadratic easing function
export const easeIn = (t: number) => t * t;
