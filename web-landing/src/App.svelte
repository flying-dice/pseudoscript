<script lang="ts">
  import { onMount } from 'svelte';
  import { initLanding } from './lib/landing-anim.js';
  import Topbar from './components/Topbar.svelte';
  import Hero from './components/Hero.svelte';
  import InstallCommand from './components/InstallCommand.svelte';
  import AgentOnramp from './components/AgentOnramp.svelte';
  import Workflow from './components/Workflow.svelte';
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

<!-- generation spine: one model, many projections — lights the active section -->
<nav class="spine" aria-label="Sections">
  <a href="#top"><span class="si">00</span><span class="sl">Model</span></a>
  <a href="#workflow"><span class="si">01</span><span class="sl">Method</span></a>
  <a href="#agents"><span class="si">02</span><span class="sl">Agent</span></a>
</nav>

<main id="top" class="shell">
  <Hero />
  <section id="install" class="install-section" data-screen-label="Install">
    <div class="wrap"><InstallCommand /></div>
  </section>
  <Workflow />
  <AgentOnramp />
  <Cta />
  <SiteFooter />
</main>

<Tweaks />
