<script>
  // Sticky site header: drawer hamburger (mobile), brand, the page nav, the
  // search trigger, and the theme toggle. Pure SSR markup — behaviors.js wires
  // every button by class.
  let { site, nav = [] } = $props();
</script>

<header class="header">
  <button
    class="hamburger"
    type="button"
    aria-expanded="false"
    aria-controls="sidebar"
    aria-label="Open navigation"
  >
    <span class="hamburger-bar"></span>
    <span class="hamburger-bar"></span>
    <span class="hamburger-bar"></span>
  </button>
  <a class="header-brand" href="{site.prefix}index.html">
    {#if site.logoFilename}<img src="{site.prefix}{site.logoFilename}" alt="" />{/if}
    <span class="header-name">{site.name}</span>
  </a>
  <nav class="header-nav" aria-label="Site">
    {#each nav as link}
      <a
        class="header-link"
        href={link.href}
        aria-current={link.current ? "page" : undefined}
      >
        {link.label}{#if link.badge}<span class="nav-badge">{link.badge}</span>{/if}
      </a>
    {/each}
  </nav>
  <div class="header-actions">
    <button class="search-btn" type="button" aria-label="Search">
      <span class="search-btn-label">Search</span>
      <kbd>&#8984;K</kbd>
    </button>
    <button class="theme-toggle" type="button" aria-label="Toggle theme">
      <span class="theme-toggle-icon" aria-hidden="true">&#9681;</span>
    </button>
  </div>
</header>
