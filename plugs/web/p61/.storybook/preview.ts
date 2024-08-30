import type { Preview } from "@storybook/react";
import { themes } from "@storybook/theming";
import "../src/index.css";

const preview: Preview = {
  parameters: {
    docs: {
      theme: themes.dark,
    },
    backgrounds: {
      default: "zone",
      values: [
        { name: "background", value: "#0F1A20" },
        { name: "zone", value: "#25283D" },
      ],
    },
    actions: { argTypesRegex: "^on[A-Z].*" },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
};

export default preview;
