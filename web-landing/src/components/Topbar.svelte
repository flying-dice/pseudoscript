<script>
  import { ArrowRight } from '@lucide/svelte';
  import Mark from './Mark.svelte';
</script>

<header class="topbar">
  <div class="wrap bar">
    <a class="brand" href="#top">
      <span class="dot"></span>
      <Mark />
      <span class="word">PseudoScript</span>
    </a>
    <nav>
      <a href="#converge">Language</a>
      <a href="#workflow">Workflow</a>
      <a href="#ide">IDE</a>
      <a href="#packages">Packages</a>
    </nav>
    <span class="sp"></span>
    <a class="btn btn-primary" href="http://ide.pdscript.dev/">Open the IDE <span class="btn-arrow ico"><ArrowRight strokeWidth={1.75} /></span></a>
  </div>
</header>
