import { dequal } from "dequal/lite";
import { MutableRefObject, useEffect, useRef, useState } from "react";

interface AnimationState<D, S> {
  data: D;
  animating: boolean;
  lastFrame: DOMHighResTimeStamp | undefined;
  state: S;
}

export interface CustomAnimation<D, S> {
  initialState: (data: D) => S;
  update: (elapsed: number | undefined, prev: S, data: D) => S;
  shouldAnimate: (state: S, data: D) => boolean;
}

const onFrame = <D, S>(
  time: DOMHighResTimeStamp | undefined,
  state: AnimationState<D, S>,
  setState: (state: S) => void,
  customAnimation: MutableRefObject<CustomAnimation<D, S>>,
) => {
  let elapsed: number | undefined = undefined;
  if (time === undefined) {
    state.lastFrame = undefined;
  } else {
    if (state.lastFrame === undefined) {
      state.lastFrame = time;
    } else {
      elapsed = (time - state.lastFrame) / 1000;
      state.lastFrame = time;
    }
  }

  state.state = state.state = customAnimation.current.update(
    elapsed,
    state.state,
    state.data,
  );
  setState(state.state);
  state.animating = customAnimation.current.shouldAnimate(
    state.state,
    state.data,
  );

  // Check if we need to animate
  if (state.animating) {
    requestAnimationFrame((time) => {
      onFrame(time, state, setState, customAnimation);
    });
  }
};

const useAnimation = <D, S>(animation: CustomAnimation<D, S>, data: D): S => {
  const initialState = animation.initialState(data);

  const animationState = useRef<AnimationState<D, S>>({
    data,
    animating: false,
    lastFrame: undefined,
    state: initialState,
  });
  const animationRef = useRef(animation);
  animationRef.current = animation;
  const [state, setState] = useState<S>(initialState);

  useEffect(() => {
    const shouldAnimate = animation.shouldAnimate(
      animationState.current.state,
      data,
    );
    if (!dequal(data, animationState.current.data) || shouldAnimate) {
      animationState.current.data = data;
      if (!animationState.current.animating) {
        animationState.current.lastFrame = undefined;
        animationState.current.animating = true;
        requestAnimationFrame((time) => {
          onFrame(time, animationState.current, setState, animationRef);
        });
      }
    }
  }, [animation, data, setState]);
  return state;
};

export default useAnimation;
