<script>
  // The PseudoScript logo mark: nested squares with an accent core.
  let { className = 'mark', strokeWidth = 1.4 } = $props();
</script>

<svg class={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
  <rect x="2.5" y="2.5" width="19" height="19" rx="3" stroke="currentColor" stroke-width={strokeWidth} opacity="0.45" />
  <rect x="6.5" y="6.5" width="11" height="11" rx="2" stroke="currentColor" stroke-width={strokeWidth} />
  <circle cx="12" cy="12" r="2.4" fill="var(--accent)" />
</svg>
