<script lang="ts">
  // A gallery of the shadcn-svelte Button on the IDE's design tokens — `primary`
  // is the brand vermilion, surfaces/borders come from the existing palette, the
  // radius from --radius. For Storybook (see button.stories.ts).
  import { Button } from "./index.js";
</script>

<div style="display: grid; gap: 16px; padding: 24px; background: var(--bg); color: var(--ink); font-family: var(--font-sans);">
  <div style="display: flex; gap: 12px; flex-wrap: wrap; align-items: center;">
    <Button>Default</Button>
    <Button variant="secondary">Secondary</Button>
    <Button variant="outline">Outline</Button>
    <Button variant="ghost">Ghost</Button>
    <Button variant="destructive">Destructive</Button>
    <Button variant="link">Link</Button>
  </div>
  <div style="display: flex; gap: 12px; flex-wrap: wrap; align-items: center;">
    <Button size="sm">Small</Button>
    <Button>Default</Button>
    <Button size="lg">Large</Button>
    <Button disabled>Disabled</Button>
  </div>
</div>
