<script lang="ts">
  import { onMount } from 'svelte';
  import { initLanding } from './lib/landing-anim.js';
  import Topbar from './components/Topbar.svelte';
  import Hero from './components/Hero.svelte';
  import Convergence from './components/Convergence.svelte';
  import Workflow from './components/Workflow.svelte';
  import IdeShowcase from './components/IdeShowcase.svelte';
  import Packages from './components/Packages.svelte';
  import Cta from './components/Cta.svelte';
  import SiteFooter from './components/SiteFooter.svelte';
  import Tweaks from './components/Tweaks.svelte';

  onMount(() => {
    // initLanding queries the now-mounted DOM and wires the scroll-driven
    // animations; it returns a teardown that removes its listeners.
    return initLanding();
  });
</script>

<Topbar />

<main id="top" class="shell">
  <Hero />
  <Convergence />
  <Workflow />
  <IdeShowcase />
  <Packages />
  <Cta />
  <SiteFooter />
</main>

<Tweaks />
