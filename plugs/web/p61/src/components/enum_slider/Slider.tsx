import useSlider from "music-ui/enum-slider/useSlider";

export interface Props {
  index: number | undefined;
  count: number;
  selectIndex: (index: number) => void;
  onGrabOrRelease?: (grabbed: boolean) => void;
}

const BALL_SIZE = 12;

const Slider = ({
  index,
  count,
  selectIndex: selectIndex,
  onGrabOrRelease,
}: Props) => {
  const {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    containerRef,
    ballRef,
    ball,
  } = useSlider<HTMLDivElement, HTMLDivElement>({
    ballMargin: 2.5,
    lineSpacing: 24,
    ballSize: BALL_SIZE,
    index,
    count,
    selectIndex,
    onGrabOrRelease,
  });

  return (
    <div className="relative flex pb-[1.5px] pt-[3.5px]">
      <div
        className="relative"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
        ref={containerRef}
      >
        <div className="border-pop relative me-[11px] ms-[11px] h-full w-[19px] rounded-full border blur-[1px]" />
        <div className="border-pop absolute inset-0 me-[11px] ms-[11px] rounded-full border">
          {ball !== undefined && (
            <div
              className="bg-border absolute left-[2.5px] rounded-full"
              data-testid="slider-knob"
              ref={ballRef}
              style={{
                width: `${BALL_SIZE}px`,
                height: `${BALL_SIZE}px`,
                bottom: `${ball.bottom}px`,
              }}
            ></div>
          )}
        </div>
      </div>
    </div>
  );
};

export default Slider;
