import { describe, expect, test, afterEach, mock } from "bun:test";
import EnumSlider from ".";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
afterEach(cleanup);
describe("EnumSlider", () => {
  test("EnumSlider shows aria value", () => {
    const testValues = ["a", "b", "c"];
    const testValue = "b";
    render(<EnumSlider value={testValue} label={"test"} values={testValues} />);
    const groupElement = screen.getByRole("radiogroup");
    expect(groupElement).toBeDefined();
    expect(groupElement.getAttribute("aria-label")).toBe("test");
    const radioElements = screen.getAllByRole("radio");
    expect(radioElements).toHaveLength(3);
    const checkedRadioElements = radioElements.filter(
      (el) => el.getAttribute("aria-checked") === "true",
    );
    expect(checkedRadioElements).toHaveLength(1);
    expect(checkedRadioElements[0].getAttribute("aria-label")).toBe("b");
  });
  test("Basic keyboard interactions", async () => {
    const testValues = ["a", "b", "c"];
    let testValue = "";
    const user = userEvent.setup();
    const onValue = mock((_: string) => undefined);

    const { rerender: rerender_ } = render(
      <EnumSlider
        value={testValue}
        label={"test"}
        values={testValues}
        onValue={onValue}
      />,
    );
    const rerender = () => {
      rerender_(
        <EnumSlider
          value={testValue}
          label={"test"}
          values={testValues}
          onValue={onValue}
        />,
      );
    };
    expect(document.activeElement).toBe(document.body);
    await user.tab();
    expect(document.activeElement?.getAttribute("role")).toBe("radio");
    expect(document.activeElement?.getAttribute("aria-label")).toBe("a");
    expect(document.activeElement?.getAttribute("aria-checked")).not.toBe(
      "true",
    );

    await user.keyboard("{Space}");
    expect(onValue).toHaveBeenLastCalledWith("a");
    expect(document.activeElement?.getAttribute("aria-checked")).not.toBe(
      "true",
    );
    testValue = "a";
    rerender();
    expect(document.activeElement?.getAttribute("aria-checked")).toBe("true");

    await user.keyboard("{ArrowRight}");
    expect(onValue).toHaveBeenLastCalledWith("b");

    // After re-rendering, the active element should be "b"
    testValue = "b";
    rerender();
    expect(document.activeElement?.getAttribute("aria-label")).toBe("b");

    // If someone else selects "a" while we're focused, focus should jump to "a".
    testValue = "a";
    rerender();
    expect(document.activeElement?.getAttribute("aria-label")).toBe("a");

    // Finally, up arrow should wrap around to "c".
    await user.keyboard("{ArrowUp}");
    expect(onValue).toHaveBeenLastCalledWith("c");
  });
  test("focus doesn't jump around when not in group", async () => {
    const testValues = ["a", "b", "c"];
    let testValue = "";
    const user = userEvent.setup();

    const { rerender: rerender_ } = render(
      <>
        <EnumSlider value={testValue} label={"test"} values={testValues} />
        <button>Other</button>
      </>,
    );
    const rerender = () => {
      rerender_(
        <>
          <EnumSlider value={testValue} label={"test"} values={testValues} />
          <button>Other</button>
        </>,
      );
    };

    expect(document.activeElement).toBe(document.body);
    testValue = "c";
    rerender();
    expect(document.activeElement).toBe(document.body);
    await user.tab();
    expect(document.activeElement).not.toBe(document.body);
    expect(document.activeElement?.getAttribute("role")).toBe("radio");
    await user.tab();
    expect(document.activeElement?.nodeName).toBe("BUTTON");
    testValue = "a";
    rerender();
    expect(document.activeElement?.nodeName).toBe("BUTTON");
  });
  test("Clicking on the handle should focus the group", async () => {
    const testValues = ["a", "b", "c"];
    let testValue = "a";
    const user = userEvent.setup();
    const onValue = mock((_: string) => undefined);

    const { rerender: rerender_ } = render(
      <EnumSlider
        value={testValue}
        label={"test"}
        values={testValues}
        onValue={onValue}
      />,
    );
    const rerender = () => {
      rerender_(
        <EnumSlider
          value={testValue}
          label={"test"}
          values={testValues}
          onValue={onValue}
        />,
      );
    };

    await user.click(screen.getByTestId("slider-knob"));
    const newValue = onValue.mock.lastCall?.[0];
    expect(newValue).not.toBeUndefined();
    testValue = newValue!;

    rerender();

    const nextIndex = (testValues.indexOf(testValue) + 1) % testValues.length;

    await user.keyboard("{ArrowRight}");
    expect(onValue).toHaveBeenLastCalledWith(testValues[nextIndex]);
  });
});
