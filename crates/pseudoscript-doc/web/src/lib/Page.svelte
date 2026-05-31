<script>
  import { onMount } from "svelte";
  import Sidebar from "./Sidebar.svelte";
  import IndexContent from "./IndexContent.svelte";
  import ModuleContent from "./ModuleContent.svelte";
  import DocContent from "./DocContent.svelte";
  import Footer from "./Footer.svelte";
  import { initBehaviors } from "./behaviors.js";

  let { site, docGroups = [], sidebar, page } = $props();

  // Tree collapse, search filter, and active-node highlight are imperative DOM
  // wiring; they run only after hydration, never under QuickJS SSR.
  onMount(() => initBehaviors(document));
</script>

<div class="layout">
  <Sidebar {site} {docGroups} {sidebar} />
  <main class="main">
    <div class="content">
      {#if page.kind === "index"}
        <IndexContent {page} />
      {:else if page.kind === "doc"}
        <DocContent {page} />
      {:else}
        <ModuleContent {page} />
      {/if}
      <Footer />
    </div>
  </main>
</div>
