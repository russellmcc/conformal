import { useId, useMemo } from "react";
import * as d3shape from "d3-shape";
import { easeIn, fade, useAnimation } from "music-ui/animation";

interface ResolvedProps {
  size: number;
  innerRadiusRatio: number;
}

const defaultProps: ResolvedProps = {
  size: 61,
  innerRadiusRatio: 18 / 30.5,
};

export interface Data {
  value: number;

  hover: boolean;

  grabbed: boolean;
}

interface State {
  hoverEase: number;
  grabbedEase: number;
  phases: number[];
}

const TAU = Math.PI * 2;
const startAngle = -TAU * (3 / 8);
const popColor = "#F26DF9";
const borderColor = "#6290C8";
const bgColor = "#0F1A20";
const baseStrokeWidth = 1;
const hoverEaseTime = 1.0;
const grabbedEaseTime = 1.0;

// These are in units of TAU
const restPhases = [3 / 16, 7 / 16, 11 / 16, 13 / 16];
// These are in units of innerRadius
const radii = [5 / 16, 3 / 16, 7 / 16, 11 / 16];
// These are multipliers for baseSpeed
const speeds = [2 / 8, -1 / 8, -5 / 8, 3 / 8];
// TAU/ second
const baseSpeed = 0.2;

const baseOpacity = 0.1;
const hoverOpacity = 0.25;

const SINGLE_EDGE_PATH_BREAKPOINT = 31;

const getStrokeWidth = (size: number) => {
  if (size <= SINGLE_EDGE_PATH_BREAKPOINT) {
    return baseStrokeWidth * 2;
  }
  return baseStrokeWidth;
};

const getSingleEdgePath = ({ innerRadiusRatio, size }: ResolvedProps) => {
  const outerRadius = size / 2;
  const innerRadius = outerRadius * innerRadiusRatio;
  const strokeWidth = getStrokeWidth(size);
  // Note that we assume that the edge is horizontally mirrored.
  const fillStartOuter = {
    x: outerRadius + Math.cos(startAngle) * outerRadius,
    y: outerRadius - Math.sin(startAngle) * outerRadius,
  };
  const fillStartInner = {
    x: outerRadius + Math.cos(startAngle) * (innerRadius - strokeWidth / 2),
    y: outerRadius - Math.sin(startAngle) * (innerRadius - strokeWidth / 2),
  };

  // adjust the starting point so that the stroke aligns with the fill arc
  const adjustment = {
    x: (-Math.cos(startAngle) * strokeWidth) / 2,
    y: (-Math.sin(startAngle) * strokeWidth) / 2,
  };
  const startOuter = {
    x: fillStartOuter.x + adjustment.x / 2,
    y: fillStartOuter.y + adjustment.y / 2,
  };
  const startInner = {
    x: fillStartInner.x + adjustment.x / 2,
    y: fillStartInner.y + adjustment.y / 2,
  };

  return `M ${startOuter.x} ${startOuter.y} L ${startInner.x} ${startInner.y}
      A ${innerRadius - strokeWidth / 2} ${innerRadius - strokeWidth / 2} 0 1 1 ${2 * outerRadius - startInner.x} ${startInner.y}
      L ${2 * outerRadius - startOuter.x} ${startOuter.y}`;
};

const getEdgePath = ({ innerRadiusRatio, size }: ResolvedProps) => {
  if (size <= SINGLE_EDGE_PATH_BREAKPOINT) {
    return getSingleEdgePath({ innerRadiusRatio, size });
  }
  const outerRadius = size / 2;
  const innerRadius = outerRadius * innerRadiusRatio;
  const strokeWidth = getStrokeWidth(size);
  // Note that we assume that the edge is horizontally mirrored.
  const fillStartOuter = {
    x: outerRadius + Math.cos(startAngle) * (outerRadius - strokeWidth / 2),
    y: outerRadius - Math.sin(startAngle) * (outerRadius - strokeWidth / 2),
  };
  const fillStartInner = {
    x: outerRadius + Math.cos(startAngle) * (innerRadius - strokeWidth / 2),
    y: outerRadius - Math.sin(startAngle) * (innerRadius - strokeWidth / 2),
  };

  // adjust the starting point so that the stroke aligns with the fill arc
  const adjustment = {
    x: -Math.cos(startAngle) * strokeWidth,
    y: -Math.sin(startAngle) * strokeWidth,
  };
  const startOuter = {
    x: fillStartOuter.x + adjustment.x / 2,
    y: fillStartOuter.y + adjustment.y / 2,
  };
  const startInner = {
    x: fillStartInner.x + adjustment.x / 2,
    y: fillStartInner.y + adjustment.y / 2,
  };

  const lowerOuter = {
    x: fillStartOuter.x + (adjustment.x * 5) / 2,
    y: fillStartOuter.y + (adjustment.y * 5) / 2,
  };

  // Note this assumes our original angle is fully diagonal
  const lowerInner = {
    x: lowerOuter.x - (startInner.y - lowerOuter.y),
    y: startInner.y,
  };

  const lowerRadius = innerRadius - (strokeWidth * 5) / 2;
  const lowerArcStart = {
    x: outerRadius + Math.cos(startAngle) * lowerRadius,
    y: outerRadius - Math.sin(startAngle) * lowerRadius,
  };

  return `M ${startOuter.x} ${startOuter.y} L ${startInner.x} ${startInner.y}
      A ${innerRadius - strokeWidth / 2} ${innerRadius - strokeWidth / 2} 0 1 1 ${2 * outerRadius - startInner.x} ${startInner.y}
      L ${2 * outerRadius - startOuter.x} ${startOuter.y}
      L ${2 * outerRadius - lowerOuter.x} ${lowerOuter.y}
      L ${2 * outerRadius - lowerInner.x} ${lowerInner.y}
      L ${2 * outerRadius - lowerArcStart.x} ${lowerArcStart.y}
      A ${lowerRadius} ${lowerRadius} 0 1 0 ${lowerArcStart.x} ${lowerArcStart.y}
      L ${lowerInner.x} ${lowerInner.y}
      L ${lowerOuter.x} ${lowerOuter.y}
      Z`;
};

export interface Props {
  /** Knobs always have square dimension, this represents one side. */
  size?: number;

  /** The inner radius of the knob track, as a ratio of the full radius */
  innerRadiusRatio?: number;

  /** Currrent value of the knob */
  value: number;

  /** True if the knob is grabbed */
  grabbed: boolean;

  /** True if the knob is hovered */
  hover: boolean;
}

const animation = {
  initialState: (data: Data): State => ({
    hoverEase: data.hover ? 1 : 0,
    grabbedEase: data.grabbed ? 1 : 0,
    phases: [0, 0, 0, 0],
  }),
  update: (elapsed: number | undefined, prev: State, data: Data): State =>
    elapsed
      ? (() => {
          const grabbedEase = fade(
            prev.grabbedEase,
            data.grabbed,
            elapsed,
            grabbedEaseTime,
          );

          return {
            hoverEase: fade(
              prev.hoverEase,
              data.hover || data.grabbed,
              elapsed,
              hoverEaseTime,
            ),
            grabbedEase,
            phases: prev.phases.map((phase, index) =>
              grabbedEase === 0
                ? 0
                : (() => {
                    let newPhase =
                      phase + elapsed * baseSpeed * speeds[index] * grabbedEase;
                    // Wrap to ensure we're within [-0.5, 0.5] - NOTE that we can only
                    // do this if grabbedEase is 1, otherwise we're fading in the contribution of
                    // `state.phases` so we'll jump if we wrap.
                    if (grabbedEase === 1) {
                      while (newPhase > 0.5) {
                        newPhase -= 1;
                      }
                      while (newPhase < -0.5) {
                        newPhase += 1;
                      }
                    }
                    return newPhase;
                  })(),
            ),
          };
        })()
      : prev,
  shouldAnimate: (state: State, data: Data) =>
    state.hoverEase !== (data.hover || data.grabbed ? 1 : 0) ||
    data.grabbed ||
    state.grabbedEase !== 0,
};

const Display = ({
  size = defaultProps.size,
  innerRadiusRatio = defaultProps.innerRadiusRatio,
  value,
  grabbed,
  hover,
}: Props) => {
  const data = useMemo(
    () => ({
      value,
      grabbed,
      hover,
    }),
    [value, grabbed, hover],
  );
  const state = useAnimation(animation, data);

  const idPrefix = useId();

  const outerRadius = size / 2;
  const innerRadius = innerRadiusRatio * outerRadius;
  const maskId = `${idPrefix}-mask`;
  const hoverEase = easeIn(state.hoverEase);
  const grabbedEase = easeIn(state.grabbedEase);

  return (
    <svg width={`${size}px`} height={`${size}px`}>
      <mask id={maskId} style={{ maskType: "alpha" }}>
        <path
          fill="white"
          d={
            d3shape.arc()({
              startAngle,
              endAngle: -TAU * (3 / 8) + ((data.value / 100) * TAU * 3) / 4,
              innerRadius,
              outerRadius,
            })!
          }
          transform={`translate(${outerRadius}, ${outerRadius})`}
        />
      </mask>
      <path
        stroke={popColor}
        fill="none"
        strokeWidth={getStrokeWidth(size)}
        d={getEdgePath({ size, innerRadiusRatio })}
      />
      <defs>
        <filter id={`${idPrefix}-blur`}>
          <feGaussianBlur
            in="SourceGraphic"
            stdDeviation={(1 - hoverEase) * 1 + 1}
          />
        </filter>
      </defs>
      <g mask={`url(#${maskId})`}>
        <circle
          cx={outerRadius}
          cy={outerRadius}
          r={outerRadius}
          fill={borderColor}
        />
        {...state.phases.map((phaseRaw, index) => {
          const phase = TAU * (restPhases[index] + phaseRaw * grabbedEase);

          return (
            <g
              key={`circle-${index}`}
              style={{ mixBlendMode: index % 2 ? "lighten" : "darken" }}
            >
              <circle
                fill={index % 2 ? borderColor : bgColor}
                cx={
                  outerRadius +
                  hoverEase * Math.cos(phase) * innerRadius * radii[index]
                }
                cy={
                  outerRadius -
                  hoverEase * Math.sin(phase) * innerRadius * radii[index]
                }
                r={outerRadius}
                opacity={baseOpacity + hoverEase * (hoverOpacity - baseOpacity)}
              />
            </g>
          );
        })}
      </g>
      <path
        stroke={popColor}
        fill="none"
        strokeWidth={getStrokeWidth(size)}
        d={getEdgePath({ size, innerRadiusRatio })}
        filter={`url(#${idPrefix}-blur)`}
      />
    </svg>
  );
};

export default Display;
