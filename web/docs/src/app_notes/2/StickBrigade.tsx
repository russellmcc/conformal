"use client";
import { useEffect, useRef, useState } from "react";
import { clockRateAtom, numBucketsAtom } from "./Controls";
import { useAtom } from "jotai";

const INPUT_FREQUENCY = 1 / 17;
const GRAPH_LENGTH = 100;
const GRAPH_SCALE = 25;
const RIPPLE_MULT = 8;
const RIPPLE_SPATIAL_RADIANS = -0.24;
const RIPPLE_AMPLITUDE = 20;
const ANIMATION_SPEED = 1.0;

const Bucket = ({
  x,
  y,
  fullness,
  fill,
  opacity,
}: {
  x: number;
  y: number;
  fullness: number;
  fill: string;
  opacity: number;
}) => {
  // Calculate the height of the liquid based on fullness (0 to 1)
  const liquidHeight = 30 * fullness;
  const liquidY = 30 - liquidHeight;

  return (
    <>
      <defs>
        <mask id={`bucketMask-${x}-${y}`}>
          <path
            d="M -19.5 0 L -14.5 30 L 14.5 30 L 19.5 0 Z"
            fill="white"
            transform={`translate(${x}, ${y})`}
          />
        </mask>
      </defs>

      {/* Bucket outline */}
      <path
        d="M -20 0 L -15 30 L 15 30 L 20 0 Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        transform={`translate(${x}, ${y})`}
        opacity={opacity}
      />

      {/* Handle */}
      <path
        d="M -20 0 L -20 -3 A 20 15 0 0 1 20 -3 L 20 0"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        transform={`translate(${x}, ${y})`}
        opacity={opacity}
      />

      {/* Liquid fill - using rectangle with mask */}
      <rect
        x={x - 20}
        y={y + liquidY}
        width={40}
        height={30}
        fill={fill}
        mask={`url(#bucketMask-${x}-${y})`}
        opacity={opacity}
      />
    </>
  );
};

const Graph = ({
  data,
  xPoints,
  x,
  y,
  width,
  height,
  color,
  arrowColor,
  arrowLength,
  xScale,
  xShift,
}: {
  data: number[];
  xPoints: number[];
  x: number;
  y: number;
  width: number;
  height: number;
  color: [number, number, number];
  arrowColor?: [number, number, number];
  arrowLength?: number[];
  xScale: number;
  xShift: number;
}) => {
  // Handle single point case
  if (data.length === 1) {
    const xPos = x + ((xShift - xPoints[0]!) / xScale) * width;
    const yPos = y + height - data[0]! * height;

    // Calculate arrow for single point
    const arrow = arrowLength?.[0] ? (
      <g opacity="0.5">
        <defs>
          <linearGradient
            id={`arrow-gradient-${x}-${y}-0`}
            gradientUnits="userSpaceOnUse"
            x1={xPos}
            y1={yPos}
            x2={xPos - (arrowLength[0] / xScale) * width}
            y2={yPos}
          >
            <stop
              offset="0%"
              stopColor={`rgb(${color[0]}, ${color[1]}, ${color[2]})`}
            />
            <stop
              offset="100%"
              stopColor={`rgb(${arrowColor?.[0] ?? color[0]}, ${arrowColor?.[1] ?? color[1]}, ${arrowColor?.[2] ?? color[2]})`}
            />
          </linearGradient>
        </defs>
        <line
          x1={xPos}
          y1={yPos}
          x2={xPos - (arrowLength[0] / xScale) * width}
          y2={yPos}
          stroke={`url(#arrow-gradient-${x}-${y}-0)`}
          strokeWidth="1"
        />
        <path
          d={`M ${xPos + 3} ${yPos - 3} L ${xPos} ${yPos} L ${xPos + 3} ${yPos + 3}`}
          stroke={`rgb(${color[0]}, ${color[1]}, ${color[2]})`}
          fill="none"
          strokeWidth="1"
        />
      </g>
    ) : null;

    return (
      <>
        <rect
          x={x}
          y={y}
          width={width}
          height={height}
          fill="none"
          stroke="currentColor"
          strokeWidth="0.5"
          opacity="0.3"
        />
        {arrow}
        <circle
          cx={xPos}
          cy={yPos}
          r="2"
          fill={`rgb(${color[0]}, ${color[1]}, ${color[2]})`}
        />
      </>
    );
  }

  if (data.length < 2) return null;

  // Create points for the polyline
  const points = data
    .map((value, index) => {
      const xPos = x + ((xShift - xPoints[index]!) / xScale) * width;
      const yPos = y + height - value * height;
      return `${xPos},${yPos}`;
    })
    .join(" ");

  // Calculate arrow points if arrowLength is provided
  const arrows = arrowLength
    ? data
        .map((value, index) => {
          // Get arrow length for this index
          const length = (arrowLength[index] ?? 0) / xScale;

          if (length === 0) return null;

          const xPos = x + ((xShift - xPoints[index]!) / xScale) * width;
          const yPos = y + height - value * height;
          const arrowEndX = xPos - length * width;
          const arrowWidth = arrowEndX - xPos;
          const headSize = 3;

          // Calculate gradient colors
          const targetColor = arrowColor ?? color;
          const gradientId = `arrow-gradient-${x}-${y}-${index}`;

          return (
            <g key={`arrow-${index}`} opacity="0.5">
              <defs>
                <linearGradient
                  id={gradientId}
                  gradientUnits="userSpaceOnUse"
                  x1={xPos}
                  y1={yPos}
                  x2={xPos + arrowWidth}
                  y2={yPos}
                >
                  <stop
                    offset="0%"
                    stopColor={`rgb(${color[0]}, ${color[1]}, ${color[2]})`}
                  />
                  <stop
                    offset="100%"
                    stopColor={`rgb(${targetColor[0]}, ${targetColor[1]}, ${targetColor[2]})`}
                  />
                </linearGradient>
              </defs>
              <line
                x1={xPos}
                y1={yPos}
                x2={xPos + arrowWidth}
                y2={yPos}
                stroke={`url(#${gradientId})`}
                strokeWidth="1"
              />
              <path
                d={`M ${xPos + headSize} ${yPos - headSize} L ${xPos} ${yPos} L ${xPos + headSize} ${yPos + headSize}`}
                stroke={`rgb(${color[0]}, ${color[1]}, ${color[2]})`}
                fill="none"
                strokeWidth="1"
              />
            </g>
          );
        })
        .filter(Boolean)
    : null;

  return (
    <>
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        fill="none"
        stroke="currentColor"
        strokeWidth="0.5"
        opacity="0.3"
      />
      <polyline
        points={points}
        fill="none"
        stroke={`rgb(${color[0]}, ${color[1]}, ${color[2]})`}
        strokeWidth="1"
      />
      {arrows}
    </>
  );
};

const Faucet = () => (
  <>
    <path
      d="M9,23.12h8.19v2.5h8.69v-2.5h9.79c2.32,0,4.21,1.89,4.21,4.21v4.29h-2.5v4.12h13.5v-4.12h-2.5v-4.29
	c0-7.01-5.7-12.71-12.71-12.71h-9.79v-2.5h-2.62V8.88h4.97V5.5H14.84v3.38h5.03v3.25h-2.69v2.5H9"
      stroke="currentColor"
      fill="none"
      strokeWidth="1"
      strokeMiterlimit="10"
      transform="translate(5.5, 0)"
    />
  </>
);

type StickBrigadeProps = {
  // Clocks per second
  clockRate?: number;
  children?: React.ReactNode;
  bucketCount?: number;
};

type State = {
  inputTime: number;
  bbdTime: number;
  input: number;
  buckets: number[];
  inputGraph: number[];
  inputTimes: number[];
  outputGraph: number[];
  outputTimes: number[];
  delays: number[];
};

const StickBrigade = ({ children }: StickBrigadeProps) => {
  const [bucketCount] = useAtom(numBucketsAtom);
  const [clockRate] = useAtom(clockRateAtom);
  const [state, setState] = useState<State>({
    inputTime: 0,
    bbdTime: 0,
    input: 0.5,
    buckets: Array<number>(bucketCount + 1).fill(0),
    inputGraph: [0.5],
    outputGraph: [],
    inputTimes: [0.0],
    outputTimes: [],
    delays: [],
  });
  const clockRateRef = useRef(clockRate);
  const bucketCountRef = useRef(bucketCount);
  clockRateRef.current = clockRate;
  bucketCountRef.current = bucketCount;

  useEffect(() => {
    let lastTime: number | undefined;
    let animationFrameId: number;

    const animate = (timestamp: number) => {
      if (lastTime === undefined) {
        lastTime = timestamp;
        animationFrameId = requestAnimationFrame(animate);
        return;
      }

      const deltaTime = ((timestamp - lastTime) / 1000) * ANIMATION_SPEED;
      lastTime = timestamp;
      setState((prev) => {
        const inputTime = prev.inputTime + deltaTime;
        let input = prev.input;
        let inputGraph = prev.inputGraph;
        let inputTimes = prev.inputTimes;
        let outputGraph = prev.outputGraph;
        let outputTimes = prev.outputTimes;
        let buckets = prev.buckets;
        let delays = prev.delays;
        if (bucketCountRef.current + 1 !== buckets.length) {
          buckets = Array<number>(bucketCountRef.current + 1).fill(0);
          delays = [];
          outputGraph = [];
          outputTimes = [];
          inputTimes = [];
          inputGraph = [];
        }
        const bbdTime = prev.bbdTime + clockRateRef.current * deltaTime;
        if (Math.floor(bbdTime) > Math.floor(prev.bbdTime)) {
          const newInput =
            0.5 + 0.4 * Math.sin(inputTime * 2 * Math.PI * INPUT_FREQUENCY);

          if (inputTimes.length >= buckets.length) {
            outputGraph = [...outputGraph, buckets[buckets.length - 2]!].slice(
              -GRAPH_LENGTH,
            );
            outputTimes = [...outputTimes, inputTime].slice(-GRAPH_LENGTH);

            delays = [
              ...delays,
              inputTimes[inputTimes.length - buckets.length]! -
                outputTimes[outputTimes.length - 1]!,
            ].slice(-GRAPH_LENGTH);
          }
          inputGraph = [...inputGraph, newInput].slice(-GRAPH_LENGTH);
          inputTimes = [...inputTimes, inputTime].slice(-GRAPH_LENGTH);

          buckets = [input, ...buckets].slice(0, bucketCountRef.current + 1);
          input = newInput;
        }
        return {
          inputTime,
          bbdTime,
          input,
          inputGraph,
          outputGraph,
          buckets,
          inputTimes,
          outputTimes,
          delays,
        };
      });
      animationFrameId = requestAnimationFrame(animate);
    };

    animationFrameId = requestAnimationFrame(animate);

    return () => {
      cancelAnimationFrame(animationFrameId);
    };
  }, []);

  let bucketXShift = 0;
  let bucketYShift = 0;

  const withinCycle = state.bbdTime % 1.0;
  const spacing = 400 / bucketCountRef.current;

  if (withinCycle < 0.5) {
    const t = withinCycle * 2;
    bucketXShift = -(spacing / 2) + -(spacing / 2) * Math.cos(t * Math.PI);
    bucketYShift = 0;
  }

  const rgbInput: [number, number, number] = [0, 178, 249];
  const rgbOutput: [number, number, number] = [255, 132, 0];
  return (
    <>
      <svg
        viewBox="0 0 500 100"
        width="100%"
        style={{ display: "block", marginTop: "20px" }}
      >
        <Faucet />
        <rect
          x={47}
          y={36.5}
          width={5}
          height={64}
          fill="url(#liquidGradient)"
          opacity={
            withinCycle < 0.5 ? 0 : Math.sin((withinCycle - 0.5) * 2 * Math.PI)
          }
        />
        <defs>
          <linearGradient id="liquidGradient" gradientTransform="rotate(90)">
            {[0, 1, 2, 3, 4].map((i) => {
              const phase =
                withinCycle * RIPPLE_MULT + RIPPLE_SPATIAL_RADIANS * i;
              const rippleEffect = Math.sin(phase * Math.PI) * RIPPLE_AMPLITUDE;
              return (
                <stop
                  key={i}
                  offset={`${i * 25}%`}
                  stopColor={`rgb(${rgbInput[0] + rippleEffect}, ${
                    rgbInput[1] + rippleEffect
                  }, ${rgbInput[2] + rippleEffect})`}
                />
              );
            })}
          </linearGradient>
        </defs>
        <Bucket
          x={50 + bucketXShift}
          y={70 - bucketYShift}
          fullness={
            state.input * (withinCycle > 0.5 ? (withinCycle - 0.5) * 2 : 0)
          }
          fill={`rgb(${rgbInput[0]}, ${rgbInput[1]}, ${rgbInput[2]})`}
          opacity={withinCycle < 0.5 ? withinCycle * 2 : 1.0}
        />
        {state.buckets.map((bucket, index) => {
          const x = 50 + (index + 1) * spacing + bucketXShift;
          const xMin = 50;
          const xMax = 450;
          const [r, g, b] = [0, 1, 2].map(
            (i) =>
              ((x - xMin) / (xMax - xMin)) * (rgbOutput[i]! - rgbInput[i]!) +
              rgbInput[i]!,
          );
          return (
            <Bucket
              key={`bucket-${index}`}
              x={x}
              y={70 - bucketYShift}
              fullness={bucket}
              fill={`rgb(${r}, ${g}, ${b})`}
              opacity={
                index === bucketCountRef.current
                  ? withinCycle < 0.5
                    ? 1.0 - withinCycle * 2.0
                    : 0.0
                  : 1.0
              }
            />
          );
        })}
      </svg>
      {children}
      <svg style={{ marginTop: "20px" }} viewBox="0 0 500 80" width="100%">
        <Graph
          data={state.inputGraph}
          xPoints={state.inputTimes}
          xScale={GRAPH_SCALE}
          xShift={state.inputTime}
          x={0}
          y={0}
          width={500}
          height={80}
          color={rgbInput}
        />
        <Graph
          data={state.outputGraph}
          xPoints={state.outputTimes}
          xScale={GRAPH_SCALE}
          xShift={state.inputTime}
          x={0}
          y={0}
          width={500}
          height={80}
          color={rgbOutput}
          arrowLength={state.delays}
          arrowColor={rgbInput}
        />
      </svg>
    </>
  );
};

export default StickBrigade;
