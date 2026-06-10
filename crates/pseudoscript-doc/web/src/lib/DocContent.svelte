<script>
  // An authored Markdown page (`[[doc.sidebar]]`). The body is rendered to HTML
  // in Rust (pulldown-cmark) and trusted project content, so it mounts with
  // `{@html}` rather than being re-escaped. Prose styling lives in the global
  // sheet (`.doc-prose`) — component styles would be dropped by the SSR build.
  let { page } = $props();
</script>

<header class="page-head">
  <div class="eyebrow">Documentation</div>
  <h1>{page.title}</h1>
</header>

<article class="doc-prose">
  {@html page.html}
</article>
