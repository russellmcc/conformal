import EnumSlider, { Props } from ".";
import { Decorator } from "@storybook/react";
import { useArgs } from "@storybook/preview-api";

const GrabDecorator: Decorator<Props> = (Story, context) => {
  const updateArgs = useArgs()[1];
  const onGrabOrRelease = (grabbed: boolean) => {
    updateArgs({ grabbed });
  };
  return Story({ ...context, args: { ...context.args, onGrabOrRelease } });
};

const ValueDecorator: Decorator<Props> = (Story, context) => {
  const updateArgs = useArgs()[1];
  const onValue = (value: string) => {
    if (context.args.value === value) return;
    updateArgs({ value });
  };
  return Story({ ...context, args: { ...context.args, onValue } });
};

const demoValues = ["saw", "pulse", "pwm"];

export default {
  component: EnumSlider,
  decorators: [GrabDecorator, ValueDecorator],
  title: "EnumSlider",
  tags: ["autodocs"],
  argTypes: {
    values: {
      table: {
        disable: true,
      },
    },
    value: {
      type: "radio",
      options: demoValues,
    },
  },
};

export const Default = {
  args: {
    value: "pulse",
    values: demoValues,
    label: "shape",
  },
};
