<script>
  import SvgFigure from "./SvgFigure.svelte";
  let { page, prefix = "" } = $props();

  const stats = $derived([
    { label: "systems", value: page.stats.systems },
    { label: "containers", value: page.stats.containers },
    { label: "components", value: page.stats.components },
    { label: "flows", value: page.stats.flows },
    { label: "findings", value: page.stats.findings },
  ]);
</script>

<header class="page-head">
  <div class="eyebrow">Architecture documentation</div>
  <h1>{page.title}</h1>
  <p class="lead">
    A C4 model of the workspace: persons, systems, and their containers and
    components, with relationships and sequence flows.
  </p>
</header>

<ul class="stats">
  {#each stats as stat}
    <li>
      <span class="stat-value">{stat.value}</span>
      <span class="stat-label">{stat.label}</span>
    </li>
  {/each}
</ul>

<SvgFigure diagram={page.contextDiagram} />

<section class="card-grid">
  {#each page.cards as card}
    <a class="card" href={card.href}>
      <div class="card-title">{card.name}</div>
      <div class="card-meta">{card.meta}</div>
    </a>
  {/each}
</section>

<section class="tiles">
  <a class="tile" href="{prefix}universe.html">
    <div class="tile-title">3D Universe</div>
    <div class="tile-meta">The whole model as an explorable scene</div>
  </a>
  <a class="tile" href="{prefix}health.html">
    <div class="tile-title">Architecture health</div>
    <div class="tile-meta">Errors, warnings, and principle lints</div>
  </a>
</section>
