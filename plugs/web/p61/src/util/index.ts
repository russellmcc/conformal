export const range = function* (max: number) {
  for (let i = 0; i < max; i++) {
    yield i;
  }
};

export const skip = function* <T>(
  iterable: Iterable<T>,
  n: number,
): Generator<T> {
  const iter = iterable[Symbol.iterator]();
  while (n-- > 0) {
    const result = iter.next();
    if (result.done) {
      return;
    }
  }
  // Yield the rest of the values
  while (true) {
    const result = iter.next();
    if (result.done) {
      return;
    }
    yield result.value;
  }
};

export const map = function* <T, U>(
  iterable: Iterable<T>,
  f: (v: T) => U,
): Generator<U> {
  for (const v of iterable) {
    yield f(v);
  }
};
