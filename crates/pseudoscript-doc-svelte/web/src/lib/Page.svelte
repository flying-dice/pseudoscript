<script>
  import { onMount } from "svelte";
  import Sidebar from "./Sidebar.svelte";
  import IndexContent from "./IndexContent.svelte";
  import ModuleContent from "./ModuleContent.svelte";
  import Footer from "./Footer.svelte";
  import { initBehaviors } from "./behaviors.js";

  let { site, sidebar, page } = $props();

  // Tree collapse, search filter, and active-node highlight are imperative DOM
  // wiring; they run only after hydration, never under QuickJS SSR.
  onMount(() => initBehaviors(document));
</script>

<div class="layout">
  <Sidebar {site} {sidebar} />
  <main class="main">
    <div class="content">
      {#if page.kind === "index"}
        <IndexContent {page} />
      {:else}
        <ModuleContent {page} />
      {/if}
      <Footer />
    </div>
  </main>
</div>
