import { DragState, useDrag } from "@use-gesture/react";
import { useCallback, useRef, useState } from "react";
import { clamp } from "music-ui/util";

const KEYBOARD_STEP = 10;
const BIG_KEYBOARD_STEP = 25;

export interface GestureProps {
  value: number;
  onGrabOrRelease?: (grabbed: boolean) => void;
  onValue?: (value: number) => void;
}

const sensitivity = 1.0;

const useGesture = ({ value, onGrabOrRelease, onValue }: GestureProps) => {
  const valueSnapshot = useRef<number | undefined>(undefined);
  const lastValue = useRef<number>(value);
  lastValue.current = value;
  const grabCallback = useCallback(
    ({ active, movement }: DragState) => {
      if (active && valueSnapshot.current === undefined) {
        valueSnapshot.current = lastValue.current;
      } else if (!active) {
        valueSnapshot.current = undefined;
      }

      if (valueSnapshot.current !== undefined) {
        const diff = movement[1] * -sensitivity;
        const newValue = Math.min(
          100,
          Math.max(0, valueSnapshot.current + diff),
        );

        onValue?.(newValue);
      }
      onGrabOrRelease?.(active);
    },
    [onGrabOrRelease, onValue],
  );

  const bindDrag = useDrag(grabCallback, {
    transform: ([x, y]) => [x, y],
    pointer: {
      keys: false,
    },
  });

  const [hover, setHover] = useState(false);
  const [interacted, setInteracted] = useState(false);
  const onMouseEnter = useCallback(() => {
    setHover(true);
  }, []);
  const onMouseLeave = useCallback(() => {
    setHover(false);
  }, []);
  const onBlur = useCallback(() => {
    setInteracted(false);
  }, []);

  const onKeyDown: React.KeyboardEventHandler = useCallback(
    (event) => {
      const setValue = (v: number) => {
        onValue?.(v);
        setInteracted(true);
        event.preventDefault();
        event.stopPropagation();
      };
      switch (event.code) {
        case "ArrowUp":
        case "ArrowRight":
          setValue(clamp(value + KEYBOARD_STEP, 0, 100));
          break;
        case "ArrowDown":
        case "ArrowLeft":
          setValue(clamp(value - KEYBOARD_STEP, 0, 100));
          break;
        case "PageUp":
          setValue(clamp(value + BIG_KEYBOARD_STEP, 0, 100));
          break;
        case "PageDown":
          setValue(clamp(value - BIG_KEYBOARD_STEP, 0, 100));
          break;
        case "End":
          setValue(100);
          break;
        case "Home":
          setValue(0);
          break;
      }
    },
    [onValue, value],
  );

  return {
    hover: hover || interacted,
    props: {
      ...bindDrag(),
      onMouseEnter,
      onMouseLeave,
      onBlur,
      onKeyDown,
    },
  };
};

export default useGesture;
