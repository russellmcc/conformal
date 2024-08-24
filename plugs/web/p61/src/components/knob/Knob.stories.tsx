import Knob, { Props } from ".";
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
  const onValue = (value: number) => {
    updateArgs({ value });
  };
  return Story({ ...context, args: { ...context.args, onValue } });
};

export default {
  component: Knob,
  decorators: [GrabDecorator, ValueDecorator],
  title: "Knob",
  tags: ["autodocs"],
  argTypes: {
    value: {
      control: {
        type: "range",
        min: 0,
        max: 100,
        step: 1,
      },
    },
  },
};

export const Default = {
  args: {
    value: 50,
    grabbed: false,
    label: "knob",
    showLabel: false,
  },
};

export const Labeled = {
  args: {
    value: 50,
    grabbed: false,
    label: "width",
    valueFormatter: (value: number) => `${value.toFixed(0)}%`,
  },
};

export const SecondaryLabeled = {
  args: {
    value: 50,
    grabbed: false,
    label: "key",
    style: "secondary",
    valueFormatter: (value: number) => `${value.toFixed(0)}%`,
  },
};
