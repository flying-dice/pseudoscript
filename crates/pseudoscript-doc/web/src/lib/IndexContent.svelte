<script>
  import Diagram from "./Diagram.svelte";
  let { page } = $props();
</script>

<header class="page-head">
  <div class="eyebrow">Architecture documentation</div>
  <h1>{page.title}</h1>
  <p class="lead">
    A C4 model of the workspace: persons, systems, and their containers and
    components, with relationships and animated sequence flows.
  </p>
</header>

<Diagram diagram={page.contextDiagram} />

<section class="card-grid">
  {#each page.cards as card}
    <a class="card" href={card.href}>
      <div class="card-title">{card.name}</div>
      <div class="card-meta">{card.meta}</div>
    </a>
  {/each}
</section>
