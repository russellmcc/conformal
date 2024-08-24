import { LegacyRef, useCallback, useMemo, useRef, useState } from "react";
import { useAnimation } from "../animation";
import { clamp } from "../util";

interface Data {
  index: number | undefined;

  touchBottom: number | undefined;

  count: number;
}

interface State {
  bottom: number | undefined;
  touch:
    | {
        base: number;
        offset: number;
      }
    | undefined;
}

interface Props {
  ballMargin: number;
  lineSpacing: number;
  ballSize: number;
  index: number | undefined;
  count: number;
  selectIndex: (index: number) => void;
  onGrabOrRelease?: (grabbed: boolean) => void;
}

export interface Output<Container extends Element, Ball extends Element> {
  containerRef: LegacyRef<Container>;
  ballRef: LegacyRef<Ball>;
  onPointerDown: (event: React.PointerEvent) => void;
  onPointerMove: (event: React.PointerEvent) => void;
  onPointerUp: (event: React.PointerEvent) => void;
  onPointerCancel: (event: React.PointerEvent) => void;
  ball: { bottom: number } | undefined;
}

const RATE = 10;

export const useSlider = <Container extends Element, Ball extends Element>({
  ballMargin,
  lineSpacing,
  ballSize,
  index,
  count,
  selectIndex,
  onGrabOrRelease,
}: Props): Output<Container, Ball> => {
  const indexToBottom = useCallback(
    (index: number, count: number) =>
      ballMargin + lineSpacing * (count - 1 - index),
    [ballMargin, lineSpacing],
  );
  const target = useCallback(
    (data: Data) =>
      data.touchBottom ??
      (data.index === undefined
        ? undefined
        : indexToBottom(data.index, data.count)),
    [indexToBottom],
  );
  const bottomToIndex = useCallback(
    (bottom: number) =>
      count -
      1 -
      clamp(
        Math.floor((bottom - ballMargin + lineSpacing / 2) / lineSpacing),
        0,
        count - 1,
      ),
    [ballMargin, count, lineSpacing],
  );

  const animation = useMemo(
    () => ({
      initialState: (data: Data): State => {
        const currTarget = target(data);
        return {
          bottom: currTarget,
          touch:
            data.touchBottom === undefined || currTarget === undefined
              ? undefined
              : { base: currTarget, offset: 0 },
        };
      },
      update: (elapsed: number | undefined, prev: State, data: Data): State => {
        const currTarget = target(data);

        if (elapsed === undefined) {
          return { bottom: prev.bottom, touch: prev.touch };
        }
        if (currTarget === undefined) {
          return { bottom: currTarget, touch: undefined };
        }

        if (prev.bottom === undefined) {
          if (data.touchBottom === undefined) {
            return { bottom: currTarget, touch: undefined };
          } else {
            return {
              bottom: currTarget,
              touch: { base: currTarget, offset: 0 },
            };
          }
        }

        // check for new touch
        if (prev.touch === undefined && data.touchBottom !== undefined) {
          return {
            bottom: prev.bottom,
            touch: {
              base: data.touchBottom,
              offset: 0,
            },
          };
        }

        // check for expired touch
        if (prev.touch !== undefined && data.touchBottom === undefined) {
          return {
            bottom: prev.bottom + prev.touch.offset,
            touch: undefined,
          };
        }

        const bottomTarget = prev.touch?.base ?? currTarget;
        let nextBottom =
          prev.bottom +
          Math.min(1.0, RATE * elapsed) * (bottomTarget - prev.bottom);
        if (Math.abs(nextBottom - bottomTarget) < 0.01) {
          nextBottom = bottomTarget;
        }

        if (prev.touch === undefined) {
          return {
            bottom: nextBottom,
            touch: undefined,
          };
        } else {
          return {
            bottom: nextBottom,
            touch: {
              base: prev.touch.base,
              offset: data.touchBottom! - prev.touch.base,
            },
          };
        }
      },
      shouldAnimate: (state: State, data: Data) =>
        state.touch !== undefined || state.bottom !== target(data),
    }),
    [target],
  );
  const containerRef = useRef<Container>(null);
  const ballRef = useRef<Ball>(null);
  const touches = useRef<Set<number>>(new Set());
  const [touchBottom, setTouchBottom] = useState<number | undefined>(undefined);

  const data = useMemo(
    () => ({
      index,
      touchBottom,
      count,
    }),
    [index, touchBottom, count],
  );

  const state = useAnimation(animation, data);

  const eventBottom = useCallback(
    (event: { clientY: number }) => {
      const unwarped =
        clamp(
          containerRef.current!.getBoundingClientRect().bottom -
            event.clientY -
            ballMargin -
            ballSize / 2,
          0,
          lineSpacing * (count - 1),
        ) + ballMargin;
      const center = indexToBottom(bottomToIndex(unwarped), count);
      const scale = lineSpacing / 2;
      const springDistance = (unwarped - center) / scale;
      const x =
        Math.pow(Math.abs(springDistance), 3) *
        (springDistance > 0 ? 1 : -1) *
        scale;
      return center + x;
    },
    [ballMargin, ballSize, bottomToIndex, count, indexToBottom, lineSpacing],
  );

  const onPointerDown = useCallback(
    (event: React.PointerEvent) => {
      containerRef.current!.setPointerCapture?.(event.pointerId);
      const bottom = eventBottom(event);
      event.preventDefault();
      selectIndex(bottomToIndex(bottom));
      touches.current.add(event.pointerId);
      setTouchBottom(bottom);
      if (touches.current.size === 1) {
        onGrabOrRelease?.(true);
      }
    },
    [eventBottom, selectIndex, bottomToIndex, setTouchBottom, onGrabOrRelease],
  );
  const onPointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!touches.current.has(event.pointerId)) {
        return;
      }
      event.preventDefault();
      const bottom = eventBottom(event);
      setTouchBottom(bottom);
      selectIndex(bottomToIndex(bottom));
    },
    [bottomToIndex, eventBottom, selectIndex],
  );
  const onPointerUp = (event: React.PointerEvent) => {
    if (!touches.current.has(event.pointerId)) {
      return;
    }

    event.preventDefault();
    touches.current.delete(event.pointerId);
    if (touches.current.size === 0) {
      onGrabOrRelease?.(false);
      setTouchBottom(undefined);
    }
  };
  const onPointerCancel = onPointerUp;
  return {
    containerRef,
    ballRef,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    ball: { bottom: (state.bottom ?? 0) + (state.touch?.offset ?? 0) },
  };
};

export default useSlider;
