import { Handler, useDrag } from "@use-gesture/react";
import { useCallback, useRef, useState } from "react";
import { clamp } from "music-ui/util";

const KEYBOARD_STEP = 10;
const BIG_KEYBOARD_STEP = 25;

export interface GestureProps {
  value: number;
  defaultValue?: number;
  onGrabOrRelease?: (grabbed: boolean) => void;
  onValue?: (value: number) => void;
}

const sensitivity = 1.0;
const shiftSensitivity = 0.1;

const useGesture = ({
  value,
  defaultValue,
  onGrabOrRelease,
  onValue,
}: GestureProps) => {
  const lastValue = useRef<number>(value);
  lastValue.current = value;
  const grabCallback: Handler<"drag"> = useCallback(
    ({ active, delta, memo, shiftKey }) => {
      if (memo === undefined) {
        memo = lastValue.current;
      }

      const last = memo as number;

      const diff = delta[1] * -(shiftKey ? shiftSensitivity : sensitivity);
      const newValue = Math.min(100, Math.max(0, last + diff));

      onValue?.(newValue);
      onGrabOrRelease?.(active);
      return newValue;
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

  const onDoubleClick: React.MouseEventHandler = useCallback(
    (event) => {
      if (defaultValue !== undefined) {
        event.preventDefault();
        event.stopPropagation();
        onValue?.(defaultValue);
      }
    },
    [defaultValue, onValue],
  );

  return {
    hover: hover || interacted,
    props: {
      ...bindDrag(),
      onMouseEnter,
      onMouseLeave,
      onBlur,
      onKeyDown,
      onDoubleClick,
    },
  };
};

export default useGesture;
