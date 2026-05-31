<script>
  // An authored Markdown page (`[[doc.sidebar]]`). The body is rendered to HTML
  // in Rust (pulldown-cmark) and trusted project content, so it mounts with
  // `{@html}` rather than being re-escaped.
  let { page } = $props();
</script>

<header class="page-head">
  <div class="eyebrow">Documentation</div>
  <h1>{page.title}</h1>
</header>

<article class="doc-prose">
  {@html page.html}
</article>

<style>
  .doc-prose {
    max-width: var(--measure, 72ch);
    color: var(--ink);
    line-height: 1.7;
    font-size: 0.975rem;
  }
  .doc-prose :global(h1),
  .doc-prose :global(h2),
  .doc-prose :global(h3),
  .doc-prose :global(h4) {
    font-family: var(--font-display);
    color: var(--ink);
    line-height: 1.25;
    margin: 2em 0 0.6em;
    scroll-margin-top: 1.5rem;
  }
  .doc-prose :global(h1) {
    font-size: 1.6rem;
  }
  .doc-prose :global(h2) {
    font-size: 1.3rem;
    border-bottom: 1px solid var(--line);
    padding-bottom: 0.3em;
  }
  .doc-prose :global(h3) {
    font-size: 1.1rem;
  }
  .doc-prose :global(p),
  .doc-prose :global(ul),
  .doc-prose :global(ol),
  .doc-prose :global(blockquote),
  .doc-prose :global(table) {
    margin: 0 0 1em;
  }
  .doc-prose :global(ul),
  .doc-prose :global(ol) {
    padding-left: 1.4em;
  }
  .doc-prose :global(li) {
    margin: 0.3em 0;
  }
  .doc-prose :global(a) {
    color: var(--accent);
    text-decoration: none;
  }
  .doc-prose :global(a:hover) {
    text-decoration: underline;
  }
  .doc-prose :global(code) {
    font-family: var(--font-mono);
    font-size: 0.85em;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0.1em 0.35em;
  }
  .doc-prose :global(pre) {
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.9em 1.1em;
    overflow-x: auto;
    margin: 0 0 1.2em;
  }
  .doc-prose :global(pre code) {
    background: none;
    border: none;
    padding: 0;
    font-size: 0.85rem;
  }
  .doc-prose :global(blockquote) {
    border-left: 3px solid var(--accent);
    background: var(--accent-soft);
    padding: 0.4em 1em;
    color: var(--ink-soft);
  }
  .doc-prose :global(blockquote > :last-child) {
    margin-bottom: 0;
  }
  /* GitHub-style alert callouts (`> [!NOTE]` …), coloured per kind. */
  .doc-prose :global(.callout) {
    --callout: var(--accent);
    border-left: 3px solid var(--callout);
    background: color-mix(in srgb, var(--callout) 9%, transparent);
    border-radius: var(--radius-sm);
    padding: 0.6em 1em;
    margin: 0 0 1em;
  }
  .doc-prose :global(.callout-note) {
    --callout: var(--k-person);
  }
  .doc-prose :global(.callout-tip) {
    --callout: var(--k-container);
  }
  .doc-prose :global(.callout-important) {
    --callout: var(--k-data);
  }
  .doc-prose :global(.callout-warning) {
    --callout: var(--k-component);
  }
  .doc-prose :global(.callout-caution) {
    --callout: var(--k-system);
  }
  .doc-prose :global(.callout-title) {
    margin: 0 0 0.3em;
    color: var(--callout);
    font-weight: 700;
    font-size: 0.8em;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .doc-prose :global(.callout > :last-child) {
    margin-bottom: 0;
  }
  .doc-prose :global(table) {
    border-collapse: collapse;
    width: 100%;
    font-size: 0.9rem;
  }
  .doc-prose :global(th),
  .doc-prose :global(td) {
    border: 1px solid var(--line);
    padding: 0.5em 0.7em;
    text-align: left;
  }
  .doc-prose :global(th) {
    background: var(--surface-2);
    font-weight: 600;
  }
  .doc-prose :global(hr) {
    border: none;
    border-top: 1px solid var(--line);
    margin: 2em 0;
  }
  .doc-prose :global(img) {
    max-width: 100%;
    border-radius: var(--radius-sm);
  }
</style>
