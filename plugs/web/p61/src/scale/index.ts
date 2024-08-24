/** A `Scale` is a mapping with an inverse between two [0,1] intervals (which we call "source" and "dest").
 *
 * `to` will map source to dest, and `from` will map dest to source.
 *
 * Our convention for plug-ins is to consider the "source" to be the "displayed" parameter value in the UI,
 * and the "Dest" to be the _raw_ exposed by the plug-in,
 *
 */
export interface Scale {
  to: (value: number) => number;
  from: (value: number) => number;
}

/**  An exponential scale is represented by a single break point representing the dest value at 0.5 source value
 *
 * An exponential scale will start out "slow" (big changes in the source coordinate will result in small changes in the dest coordinate)
 * */
export const exponentialScale = (fromBreak: number, toBreak: number): Scale => {
  // Special case: if the break points are near eachother, just return an identity mapping!
  if (Math.abs(fromBreak - toBreak) < 1e-6) {
    return {
      to: (value: number) => value,
      from: (value: number) => value,
    };
  }

  // Special case: of toBreak is _higher_ than fromBreak, do an inverse mapping!
  if (toBreak > fromBreak) {
    const inverse = exponentialScale(toBreak, fromBreak);
    return {
      to: (value: number) => inverse.from(value),
      from: (value: number) => inverse.to(value),
    };
  }

  if (
    toBreak < 1e-6 ||
    toBreak > fromBreak - 1e-6 ||
    fromBreak < 1e-6 ||
    fromBreak > 1 - 1e-6
  ) {
    throw new Error(
      `Invalid exponentialScale(${fromBreak}, ${toBreak}).  fromBreak and toBreak must be in the range (0, 1) and toBreak < fromBreak`,
    );
  }

  // See derivation in exponential_scale.ipynb
  const getTScale = (): number => {
    // Define our dual numbers
    interface Dual {
      val: number;
      slope: number;
    }
    const add = (a: Dual, b: number): Dual => ({
      val: a.val + b,
      slope: a.slope,
    });
    const addDual = (a: Dual, b: Dual): Dual => ({
      val: a.val + b.val,
      slope: a.slope + b.slope,
    });
    const mul = (a: Dual, b: number): Dual => ({
      val: a.val * b,
      slope: a.slope * b,
    });
    const mulDual = (a: Dual, b: Dual): Dual => ({
      val: a.val * b.val,
      slope: a.val * b.slope + a.slope * b.val,
    });
    const pow = (a: Dual, b: number): Dual => {
      console.assert(Math.abs(b) >= 1, "We don't support powers below 1!");
      return {
        val: Math.pow(a.val, b),
        slope: b * Math.pow(a.val, b - 1) * a.slope,
      };
    };

    // We set the initial case assuming toBreak is 0.5:
    // This makes the equation a quadratic that can be solved explicitly.
    // d + 1 = ddqq + 2dq + 1
    // 0 = ddqq + (2q - 1)d
    // 0 = qqd + (2q - 1)
    // d = (1 - 2q) / qq
    let guess = (1 - 2 * toBreak) / (toBreak * toBreak);
    if (Math.abs(guess) < 1e-8) {
      guess = 1;
    }
    const evalGuess = (x: number): number =>
      (1 / x) * (x + 1 - Math.pow(toBreak * x + 1, 1 / fromBreak));
    let tries = 0;
    while (tries < 100 && Math.abs(evalGuess(guess)) > 1e-8) {
      const d = { val: guess, slope: 1 };
      const attempt = mulDual(
        pow(d, -1),
        addDual(
          add(d, 1),
          mul(pow(add(mul(d, toBreak), 1), 1 / fromBreak), -1),
        ),
      );
      guess -= attempt.val / attempt.slope;
      tries++;
    }
    if (Math.abs(evalGuess(guess)) > 1e-5) {
      throw new Error(
        `Failed to calculate to an exponential scale for exponentialScale(${fromBreak}, ${toBreak})`,
      );
    }
    return guess;
  };

  const tScale = getTScale();
  const fScale = Math.log(tScale + 1);

  return {
    to: (value: number) => (Math.exp(fScale * value) - 1) / tScale,
    from: (value: number) => Math.log(value * tScale + 1) / fScale,
  };
};
