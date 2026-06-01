<script lang="ts">
  import { onMount } from 'svelte';
  import { Apple, MonitorDown, Copy, Check } from '@lucide/svelte';

  type Os = 'unix' | 'windows';

  // One-liners pull the install script straight from the latest release's
  // assets; the script then resolves the matching `pds-<target>` archive.
  const CMDS: Record<Os, string> = {
    unix: 'curl -fsSL https://github.com/flying-dice/pseudoscript/releases/latest/download/install.sh | bash',
    windows: 'irm https://github.com/flying-dice/pseudoscript/releases/latest/download/install.ps1 | iex',
  };

  let os = $state<Os>('unix'); // 'unix' = macOS/Linux, 'windows'
  let copied = $state(false);

  onMount(() => {
    // Default the toggle to the visitor's platform.
    const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
    const p = (nav.userAgentData?.platform || nav.platform || '').toLowerCase();
    if (p.includes('win')) os = 'windows';
  });

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(CMDS[os]);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      copied = false;
    }
  }
</script>

<div class="install reveal">

  <div class="install-head">
    <span class="lbl accent">Install the CLI</span>
    <span class="sp"></span>
    <div class="seg-toggle" role="tablist" aria-label="Operating system">
      <button
        role="tab"
        aria-selected={os === 'unix'}
        class:on={os === 'unix'}
        onclick={() => (os = 'unix')}
      ><Apple size={13} strokeWidth={1.75} /> macOS / Linux</button>
      <button
        role="tab"
        aria-selected={os === 'windows'}
        class:on={os === 'windows'}
        onclick={() => (os = 'windows')}
      ><MonitorDown size={13} strokeWidth={1.75} /> Windows</button>
    </div>
  </div>

  <div class="install-cmd">
    <code><span class="prompt">{os === 'windows' ? 'PS>' : '$'}</span> {CMDS[os]}</code>
    <button class="copy" onclick={copy} aria-label="Copy install command">
      {#if copied}<Check size={15} strokeWidth={2} /> Copied{:else}<Copy size={15} strokeWidth={1.75} /> Copy{/if}
    </button>
  </div>

  <p class="install-note">
    Installs <code>pds</code> to <code>~/.pseudoscript/bin</code>{os === 'windows' ? ' (%USERPROFILE%\\.pseudoscript\\bin)' : ''}, verifies the SHA-256 checksum, and prints the PATH line to add. Then run <code>pds --help</code>.
  </p>
</div>

<style>
  .install {
    margin: 2.6rem auto 0;
    max-width: 760px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-md);
    padding: 1rem 1.1rem 1.1rem;
  }
  .install-head {
    display: flex;
    align-items: center;
    gap: .8rem;
    margin-bottom: .8rem;
    flex-wrap: wrap;
  }
  .install-head .sp { flex: 1; }

  .seg-toggle {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--surface-2);
  }
  .seg-toggle button {
    display: inline-flex;
    align-items: center;
    gap: .4rem;
    padding: .4rem .7rem;
    border: none;
    background: transparent;
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: .7rem;
    cursor: pointer;
    transition: color .14s, background .14s;
  }
  .seg-toggle button + button { border-left: 1px solid var(--line-strong); }
  .seg-toggle button:hover { color: var(--ink); }
  .seg-toggle button.on { color: var(--accent-ink); background: var(--accent); }

  .install-cmd {
    display: flex;
    align-items: center;
    gap: .8rem;
    padding: .8rem .9rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--bg);
  }
  .install-cmd code {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: .8rem;
    color: var(--ink);
    overflow-x: auto;
    white-space: nowrap;
    line-height: 1.5;
  }
  .install-cmd code::-webkit-scrollbar { height: 6px; }
  .install-cmd .prompt { color: var(--accent); user-select: none; margin-right: .4rem; }

  .copy {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: .4rem;
    padding: .4rem .7rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: .7rem;
    cursor: pointer;
    transition: color .14s, border-color .14s;
  }
  .copy:hover { color: var(--ink); border-color: var(--accent); }

  .install-note {
    margin: .8rem 0 0;
    font-size: .78rem;
    color: var(--ink-faint);
    line-height: 1.55;
  }
  .install-note code {
    font-family: var(--font-mono);
    color: var(--ink-soft);
    font-size: .9em;
  }

  @media (max-width: 560px) {
    .install-cmd { flex-direction: column; align-items: stretch; }
    .copy { justify-content: center; }
  }
</style>
