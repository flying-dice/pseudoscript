/** @type {import('@storybook/sveltekit').StorybookConfig} */
const config = {
  stories: ["../src/**/*.stories.@(js|ts|svelte)"],
  addons: [],
  framework: { name: "@storybook/sveltekit", options: {} },
  core: { disableTelemetry: true },
};

export default config;
