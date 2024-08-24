import { describe, expect, test, afterEach, mock } from "bun:test";
import Knob from ".";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

afterEach(cleanup);
describe("Knob", () => {
  test("Knob shows aria value", () => {
    const testValue = 50;
    const { rerender } = render(<Knob value={testValue} label={"test knob"} />);
    const knobElement = screen.getByRole("slider");
    expect(Number(knobElement.getAttribute("aria-valuenow"))).toBe(testValue);
    rerender(<Knob value={testValue + 1} label={"test knob"} />);
    expect(Number(knobElement.getAttribute("aria-valuenow"))).toBe(
      testValue + 1,
    );
  });

  test("Knob keyboard control", async () => {
    const testValue = 50;
    const onValue = mock((_: number) => undefined);
    const user = userEvent.setup();

    render(<Knob value={50} label={"test knob"} onValue={onValue} />);
    await user.tab();
    await user.keyboard("{ArrowUp}");
    expect(onValue).toHaveBeenCalled();
    expect(onValue.mock.calls[0][0]).toBeGreaterThan(testValue);
    await user.keyboard("{ArrowDown}");
    expect(onValue).toHaveBeenCalledTimes(2);
    expect(onValue.mock.calls[1][0]).toBeLessThan(testValue);
  });

  test("Knob aria valuetext", () => {
    const testValue = 50;
    const valueFormatter = (value: number) => `${value.toFixed(0)} UNITS`;
    const { rerender } = render(
      <Knob
        value={testValue}
        label={"test knob"}
        valueFormatter={valueFormatter}
      />,
    );
    const knobElement = screen.getByRole("slider");
    expect(knobElement.getAttribute("aria-valuetext")).toBe(
      valueFormatter(testValue),
    );
    rerender(
      <Knob
        value={testValue + 1}
        label={"test knob"}
        valueFormatter={valueFormatter}
      />,
    );
    expect(knobElement.getAttribute("aria-valuetext")).toBe(
      valueFormatter(testValue + 1),
    );
  });
});
