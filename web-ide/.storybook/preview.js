import "../src/app.css";

/** @type {import('@storybook/sveltekit').Preview} */
const preview = {
  parameters: {
    controls: { matchers: { color: /(background|color)$/i, date: /Date$/i } },
  },
};

// Render stories on the IDE's dark theme (the design tokens key off `data-theme`).
if (typeof document !== "undefined") {
  document.documentElement.setAttribute("data-theme", "dark");
}

export default preview;
