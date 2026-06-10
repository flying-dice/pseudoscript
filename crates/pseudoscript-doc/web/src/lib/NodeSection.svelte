<script>
  import Relationships from "./Relationships.svelte";
  import Scenarios from "./Scenarios.svelte";
  import SvgFigure from "./SvgFigure.svelte";
  import HealthBadge from "./HealthBadge.svelte";
  let { section, prefix = "" } = $props();
</script>

<section class="node" id={section.id}>
  <div class="node-head">
    <span class="kind-badge {section.kind}">{section.kind}</span>
    <h2>{section.name} <a class="anchor" href="#{section.id}">#</a></h2>
    <span class="vis-badge">{section.visibility}</span>
  </div>
  <code class="node-fqn">{section.fqn}</code>

  <HealthBadge diagnostics={section.diagnostics} {prefix} />

  {#if section.summary}<p class="summary">{section.summary}</p>{/if}
  {#if section.extended}<p class="extended">{section.extended}</p>{/if}

  {#if section.tags.length}
    <div class="tags">
      {#each section.tags as tag}<span class="chip">{tag}</span>{/each}
    </div>
  {/if}

  <Relationships groups={section.relationships} />
  <Scenarios scenarios={section.scenarios} />

  {#each section.diagrams as diagram}<SvgFigure {diagram} />{/each}
</section>
