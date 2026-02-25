import { describe, test, afterEach } from "bun:test";
import { render, cleanup } from "@testing-library/react";
import { Root } from "./Root.tsx";

afterEach(cleanup);

describe("main", () => {
  test("App can render without throwing", () => {
    render(<Root />);
  });
});
