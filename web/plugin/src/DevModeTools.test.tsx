import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, test } from "bun:test";
import { atom, type WritableAtom } from "jotai";
import DevModeTools from "./DevModeTools";
import type { Value } from "./protocol";
import { type Family, storesFromGenericStore, type Stores } from "./stores";
import { Context } from "./stores_react";

afterEach(cleanup);

const makeStores = (initialBooleans: Record<string, boolean>): Stores => {
  const values = new Map<string, Value>(
    Object.entries(initialBooleans).map(([path, bool]) => [path, { bool }]),
  );
  const atoms = new Map<string, WritableAtom<Value, [Value], void>>();
  const generic: Family<Value> = (path) => {
    const existing = atoms.get(path);
    if (existing) {
      return existing;
    }

    const baseAtom = atom(values.get(path) ?? { bool: false });
    const valueAtom = atom(
      (get) => get(baseAtom),
      (_get, set, update: Value) => {
        values.set(path, update);
        set(baseAtom, update);
      },
    );
    atoms.set(path, valueAtom);
    return valueAtom;
  };

  return storesFromGenericStore(generic, () => atom(null, () => {}));
};

const renderDevModeTools = (isDevMode: boolean) =>
  render(
    <Context.Provider
      value={makeStores({
        "prefs/dev_mode": isDevMode,
        "prefs/use_web_dev_server": false,
      })}
    >
      <DevModeTools />
    </Context.Provider>,
  );

describe("DevModeTools", () => {
  test("prevents the default context menu after custom handlers run", () => {
    renderDevModeTools(false);

    const target = document.createElement("div");
    document.body.append(target);

    let callCount = 0;
    let sawDefaultPrevented = false;
    target.addEventListener("contextmenu", (event) => {
      callCount += 1;
      sawDefaultPrevented = event.defaultPrevented;
    });

    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
    });
    target.dispatchEvent(event);

    expect(callCount).toBe(1);
    expect(sawDefaultPrevented).toBe(false);
    expect(event.defaultPrevented).toBe(true);

    target.remove();
  });

  test("leaves the default context menu enabled in dev mode", () => {
    renderDevModeTools(true);

    const target = document.createElement("div");
    document.body.append(target);

    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
    });
    target.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);

    target.remove();
  });
});
