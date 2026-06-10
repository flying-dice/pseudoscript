<script>
  // The whole page: skip link, sticky header, sidebar + content layout. Pure
  // SSR — no onMount, no hydration; behaviors.js progressively enhances the
  // markup. `data-prefix` carries the ../-to-root prefix for client scripts
  // (search-index injection, health links).
  import Header from "./Header.svelte";
  import Breadcrumbs from "./Breadcrumbs.svelte";
  import Sidebar from "./Sidebar.svelte";
  import IndexContent from "./IndexContent.svelte";
  import ModuleContent from "./ModuleContent.svelte";
  import DocContent from "./DocContent.svelte";
  import UniverseContent from "./UniverseContent.svelte";
  import HealthContent from "./HealthContent.svelte";
  import Footer from "./Footer.svelte";

  let { site, docGroups = [], sidebar, nav = [], crumbs = [], page } = $props();
</script>

<a class="skip-link" href="#content">Skip to content</a>
<div class="shell" data-prefix={site.prefix}>
  <Header {site} {nav} />
  <div class="layout">
    <Sidebar {site} {docGroups} {sidebar} />
    <div class="backdrop" data-backdrop></div>
    <main class="main" id="content">
      <div class="content">
        <Breadcrumbs {crumbs} />
        {#if page.kind === "index"}
          <IndexContent {page} prefix={site.prefix} />
        {:else if page.kind === "doc"}
          <DocContent {page} />
        {:else if page.kind === "universe"}
          <UniverseContent {page} />
        {:else if page.kind === "health"}
          <HealthContent {page} />
        {:else}
          <ModuleContent {page} prefix={site.prefix} />
        {/if}
        <Footer />
      </div>
    </main>
  </div>
</div>
