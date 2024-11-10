import { atom } from "jotai";

// Define atoms for our controls
export const numBucketsAtom = atom(4); // Default to 4 buckets
export const clockRateAtom = atom(1.0); // Default to 1 Hz
