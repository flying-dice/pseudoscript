import type { Meta, StoryObj } from "@storybook/svelte";

import Showcase from "./button-showcase.svelte";

// The shadcn-svelte Button, rendered on the IDE's aligned theme (brand-vermilion
// `primary`, existing surfaces/borders, --radius shapes).
const meta: Meta<typeof Showcase> = {
  title: "UI/Button",
  component: Showcase,
  tags: ["autodocs"],
};

export default meta;
type Story = StoryObj<typeof Showcase>;

export const Variants: Story = {};
