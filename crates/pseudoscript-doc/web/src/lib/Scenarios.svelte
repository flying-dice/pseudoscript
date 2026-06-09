<script>
  import Diagram from "./Diagram.svelte";

  let { scenarios } = $props();
</script>

{#if scenarios.length}
  <div class="scenarios">
    <h3>Scenarios</h3>
    {#each scenarios as scenario}
      <div class="scenario">
        <div class="scenario-name">{scenario.name}</div>
        {#if scenario.summary}<p class="summary">{scenario.summary}</p>{/if}
        {#if scenario.extended}<p class="extended">{scenario.extended}</p>{/if}
        {#if scenario.tags.length}
          <div class="tags">
            {#each scenario.tags as tag}<span class="chip">{tag}</span>{/each}
          </div>
        {/if}
        <ul class="steps">
          {#each scenario.steps as step}
            <li>
              <span class="step-kw {step.keyword}">{step.keyword}</span>
              <span class="step-text">{step.text}</span>
            </li>
          {/each}
        </ul>
        <Diagram diagram={scenario.flow} />
      </div>
    {/each}
  </div>
{/if}
