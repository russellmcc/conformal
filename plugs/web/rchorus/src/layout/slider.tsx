import { SliderProps } from "music-ui/enum-slider";
import useSlider from "music-ui/enum-slider/useSlider";

const BALL_SIZE = 19;

const Slider = ({
  index,
  count,
  selectIndex: selectIndex,
  onGrabOrRelease,
}: SliderProps) => {
  const {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    containerRef,
    ballRef,
    ball,
  } = useSlider<HTMLDivElement, HTMLDivElement>({
    ballMargin: -1,
    lineSpacing: 21,
    ballSize: BALL_SIZE,
    index,
    count,
    selectIndex,
    onGrabOrRelease,
  });

  return (
    <div
      style={{
        position: "relative",
        display: "flex",
        marginRight: "11px",
        marginLeft: "11px",
      }}
    >
      <div
        style={{ position: "relative" }}
        {...{ onPointerDown, onPointerUp, onPointerCancel, onPointerMove }}
        ref={containerRef}
      >
        <div
          style={{
            position: "relative",
            height: "100%",
            width: "21px",
          }}
        >
          <div
            style={{
              width: "1px",
              backgroundColor: "var(--text-color)",
              height: "50%",
              position: "absolute",
              top: "25%",
              left: "10px",
            }}
          ></div>
        </div>
        {ball !== undefined && (
          <div
            style={{
              position: "absolute",
              height: `${BALL_SIZE}px`,
              width: `${BALL_SIZE}px`,
              bottom: `${ball.bottom}px`,
              border: "1px solid var(--highlight-color)",
              borderRadius: "4px",
            }}
            ref={ballRef}
          ></div>
        )}
      </div>
    </div>
  );
};

export default Slider;
