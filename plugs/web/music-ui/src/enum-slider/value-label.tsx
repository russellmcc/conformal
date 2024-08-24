import * as React from "react";
import { useCallback, useMemo } from "react";
import { indexOf } from "../util";

export type ValueLabelProps = React.DetailedHTMLProps<
  React.HTMLAttributes<HTMLDivElement>,
  HTMLDivElement
> & {
  checked: boolean;
  label: string;
};

export type ValueLabel = React.ExoticComponent<ValueLabelProps>;

export const ValueLabelInternal = ({
  label,
  index,
  selectedIndex,
  numValues,
  selectIndex,
  radios,
  displayFormatter,
  ClientComponent,
}: {
  label: string;
  index: number;
  selectedIndex: number | undefined;
  numValues: number;
  selectIndex?: (i: number) => void;
  radios: React.MutableRefObject<Record<number, HTMLDivElement>>;
  displayFormatter?: (value: string) => string;
  ClientComponent: ValueLabel;
}) => {
  const displayLabel = useMemo(
    () => displayFormatter?.(label) ?? label,
    [displayFormatter, label],
  );
  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Space" || e.key === " ") {
        e.preventDefault();
        e.stopPropagation();
        selectIndex?.(index);
        return;
      }
      const directions: Record<string, number> =
        document.dir === "rtl"
          ? {
              Up: -1,
              ArrowUp: -1,
              Down: 1,
              ArrowDown: 1,
              Left: 1,
              ArrowLeft: 1,
              Right: -1,
              ArrowRight: -1,
            }
          : {
              Up: -1,
              ArrowUp: -1,
              Down: 1,
              ArrowDown: 1,
              Left: -1,
              ArrowLeft: -1,
              Right: 1,
              ArrowRight: 1,
            };
      const direction = directions[e.key];
      if (direction !== undefined) {
        e.preventDefault();
        e.stopPropagation();
        const newIndex = (index + directions[e.key] + numValues) % numValues;
        selectIndex?.(newIndex);
      }
    },
    [index, numValues, selectIndex],
  );

  const onClick = useCallback(() => {
    selectIndex?.(index);
  }, [index, selectIndex]);

  const onRef = useCallback(
    (el: HTMLDivElement | null) => {
      if (el === null) {
        delete radios.current[index];
      } else {
        radios.current[index] = el;
      }
    },
    [index, radios],
  );

  const checked = selectedIndex === index;

  return (
    <ClientComponent
      className="relative"
      onClick={onClick}
      onKeyDown={onKeyDown}
      role={"radio"}
      aria-label={displayLabel}
      aria-checked={checked}
      tabIndex={(selectedIndex ?? 0) == index ? 0 : -1}
      ref={onRef}
      checked={checked}
      label={displayLabel}
    ></ClientComponent>
  );
};

export interface LabelGroupProps {
  accessibilityLabel: string;
  values: string[];
  value: string;
  displayFormatter?: (value: string) => string;
  valueLabel: ValueLabel;
  radios: React.MutableRefObject<Record<number, HTMLDivElement>>;
  selectIndex: (i: number) => void;
}

export const LabelGroup = ({
  accessibilityLabel,
  value,
  values,
  displayFormatter,
  valueLabel,
  radios,
  selectIndex,
}: LabelGroupProps) => {
  const index = indexOf(value, values);

  return (
    <div
      className="inline-block"
      role="radiogroup"
      aria-label={accessibilityLabel}
    >
      {values.map((v, i) => (
        <ValueLabelInternal
          key={`choice-${i}`}
          label={v}
          index={i}
          selectedIndex={index}
          numValues={values.length}
          selectIndex={selectIndex}
          radios={radios}
          displayFormatter={displayFormatter}
          ClientComponent={valueLabel}
        />
      ))}
    </div>
  );
};
