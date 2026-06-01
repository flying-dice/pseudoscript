<script>
  import { Container, Database, Blocks } from '@lucide/svelte';
</script>

<section id="packages" data-screen-label="Packages">
  <div class="wrap">
    <div class="pkg-grid">
      <div class="pkg-copy">
        <div class="eyebrow-row reveal"><span class="lbl accent">Package manager</span></div>
        <h2 class="statement reveal d1">Specs become <span class="hot">shareable artefacts.</span></h2>
        <p class="lede reveal d2">A built-in package manager turns every spec into a versioned unit. Publish it, depend on it, and assemble modules into a single monorepo workspace.</p>
        <div class="pkg-cards reveal d2" style="margin-top:2rem">
          <div class="pkg k-container"><span class="glyph"><Container strokeWidth={1.75} /></span><div class="meta"><div class="name">acme/inventory <span class="v">1.4.0</span></div><div class="sub">No-oversell seat pools, atomic allocation</div></div></div>
          <div class="pkg k-data"><span class="glyph"><Database strokeWidth={1.75} /></span><div class="meta"><div class="name">std/money <span class="v">2.1.0</span></div><div class="sub">Currency-tagged value objects</div></div></div>
          <div class="pkg k-component"><span class="glyph"><Blocks strokeWidth={1.75} /></span><div class="meta"><div class="name">c4/patterns <span class="v">0.9.2</span></div><div class="sub">Reusable context &amp; container shapes</div></div></div>
        </div>
      </div>
      <div class="manifest reveal d1">
        <div class="mf-head"><span class="acc" style="color:var(--accent);font-family:var(--font-mono);font-size:.8rem">◳</span><span class="fname">pds.toml</span></div>
        <pre id="manifest"></pre>
      </div>
    </div>
  </div>
</section>
