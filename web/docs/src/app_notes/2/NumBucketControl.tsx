"use client";
import { useAtom } from "jotai";
import { numBucketsAtom } from "./Controls";

export const NumBucketControl = () => {
  const [numBuckets, setNumBuckets] = useAtom(numBucketsAtom);

  return (
    <div className="flex flex-col gap-2">
      <label className="flex items-center gap-2">
        <span>Number of Buckets: {numBuckets}</span>
        <input
          type="range"
          min={2}
          max={12}
          value={numBuckets}
          onChange={(e) => {
            setNumBuckets(parseInt(e.target.value));
          }}
          style={{ marginLeft: "1rem" }}
        />
      </label>
    </div>
  );
};
export default NumBucketControl;
