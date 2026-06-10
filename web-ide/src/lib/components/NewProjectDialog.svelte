<script lang="ts">
  // The New-project dialog: name the project, choose a target folder, then pick a
  // template (Empty, or a worked example). The chosen template scaffolds a
  // `<name>` directory inside the target folder. Opened from the launcher's "New
  // project" action and stacks above it. Bespoke styling, matched to the launcher.

  // A project template: the empty starter or a bundled example.
  type Template = { id: string; name: string; description: string; moduleCount: number };

  import type { PickOutcome } from "$lib/workspace.js";

  type Props = {
    templates?: Template[];
    // Prompts for the target folder (the native picker lives in the parent); the
    // dialog stores the handle and shows its name. A cancel keeps the prior
    // choice silently; a failure renders inline under the field (model:
    // ide::PickError — cancel and failure are never conflated).
    onchoosefolder?: () => Promise<PickOutcome>;
    onpick?: (name: string, templateId: string, parent: FileSystemDirectoryHandle) => void;
    onclose?: () => void;
  };

  let { templates = [], onchoosefolder, onpick, onclose }: Props = $props();

  // The new project's name and target folder — both mandatory. A template can't be
  // chosen until both are set; the template scaffolds a `<name>` dir in `folder`.
  let name = $state("");
  let folder = $state<FileSystemDirectoryHandle | null>(null);
  let pickError = $state<string | null>(null);
  const valid = $derived(name.trim().length > 0 && folder !== null);

  async function chooseFolder() {
    const picked = await onchoosefolder?.();
    if (!picked || picked.kind === "cancelled") return; // the user's choice — keep the prior state
    if (picked.kind === "failed") {
      pickError = picked.message;
      return;
    }
    pickError = null;
    folder = picked.handle;
  }
  const choose = (templateId: string) => {
    if (valid && folder) onpick?.(name.trim(), templateId, folder);
  };
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose?.(); }}>
  <div class="dossier" role="dialog" aria-modal="true" aria-label="New project">
    <header class="head">
      <div class="brand">
        <svg class="logo" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <rect x="2.5" y="2.5" width="19" height="19" rx="4" stroke="currentColor" stroke-width="1.4" opacity="0.45" />
          <rect x="6.5" y="6.5" width="11" height="11" rx="2.6" stroke="currentColor" stroke-width="1.5" />
          <circle cx="12" cy="12" r="2.3" fill="var(--accent)" />
        </svg>
        <span class="word">PseudoScript</span>
      </div>
      <button class="x" onclick={() => onclose?.()} aria-label="Close">✕</button>
    </header>

    <label class="namefield">
      <span class="namelabel">Name <span class="req" aria-hidden="true">*</span></span>
      <input
        class="newname"
        data-testid="new-project-name"
        bind:value={name}
        placeholder="Name your project — e.g. payments-platform"
        aria-label="New project name"
        required
      />
    </label>

    <div class="folderfield">
      <span class="namelabel">Target folder <span class="req" aria-hidden="true">*</span></span>
      <button
        class="folderpick"
        class:chosen={folder}
        data-testid="choose-folder"
        aria-invalid={pickError != null}
        onclick={chooseFolder}
      >
        <span class="folderpick-glyph" aria-hidden="true">▢</span>
        <span class="folderpick-text">{folder ? folder.name : "Choose a folder…"}</span>
        <span class="folderpick-action" aria-hidden="true">{folder ? "Change" : "Browse"}</span>
      </button>
      {#if pickError}
        <p class="fielderror" data-testid="pick-error" role="alert">Couldn't open the folder picker: {pickError}</p>
      {/if}
      <p class="fieldhint">A <code>{name.trim() || "<name>"}</code> directory is created inside it for the project.</p>
    </div>

    <h2 class="kicker">Template</h2>
    <p class="hint" class:armed={valid}>
      {valid
        ? "Choose a template to scaffold the project."
        : "Enter a name and choose a target folder to pick a template."}
    </p>
    <ul class="cards">
      {#each templates as t (t.id)}
        <li>
          <button class="card" data-testid="template-{t.id}" disabled={!valid} onclick={() => choose(t.id)}>
            <span class="ct tl"></span><span class="ct br"></span>
            <span class="card-top">
              <span class="card-name">{t.name}</span>
              <span class="count">{t.moduleCount} module{t.moduleCount === 1 ? "" : "s"}</span>
            </span>
            <span class="desc">{t.description}</span>
            <span class="go">Use template <span class="arr" aria-hidden="true">→</span></span>
          </button>
        </li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    /* one notch above the launcher (z-index 200) so it stacks on top */
    z-index: 201;
    display: grid;
    place-items: center;
    padding: 2rem;
    background: color-mix(in srgb, var(--bg) 78%, transparent);
    backdrop-filter: blur(7px) saturate(1.1);
    animation: fade 0.18s ease-out;
  }
  @keyframes fade { from { opacity: 0; } to { opacity: 1; } }

  .dossier {
    position: relative;
    width: min(54rem, 100%);
    max-height: calc(100vh - 4rem);
    overflow-y: auto;
    padding: 1.9rem 2rem 2rem;
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--surface) 96%, transparent), color-mix(in srgb, var(--surface) 88%, transparent)),
      var(--glow);
    background-color: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg), 0 0 0 1px var(--line);
    animation: dossier-in 0.34s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes dossier-in {
    from { opacity: 0; transform: translateY(14px) scale(0.985); }
    to { opacity: 1; transform: none; }
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    animation: rise 0.4s 0.02s both;
  }
  .brand { display: flex; align-items: baseline; gap: 0.55rem; }
  .brand .logo {
    width: 20px; height: 20px; align-self: center;
    color: var(--ink-soft);
  }
  .brand .word { font-family: var(--font-display); font-weight: 700; font-size: 1.04rem; letter-spacing: -0.025em; }
  .x {
    width: 1.75rem; height: 1.75rem; display: grid; place-items: center;
    background: transparent; border: none;
    border-radius: var(--radius-sm); color: var(--ink-faint); font-size: 0.9rem;
    transition: background 0.12s, color 0.12s;
  }
  .x:hover { background: var(--surface-2); color: var(--ink); }

  .kicker {
    margin: 0 0 0.85rem;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    display: flex; align-items: center; gap: 0.6rem;
  }
  .kicker::after { content: ""; flex: 1; height: 1px; background: var(--line); }

  .namefield { display: block; margin: 1.6rem 0 1.1rem; animation: rise 0.4s 0.12s both; }
  .namelabel {
    display: block; margin-bottom: 0.3rem;
    font-family: var(--font-mono); font-size: 0.62rem; letter-spacing: 0.16em;
    text-transform: uppercase; color: var(--ink-faint);
  }
  .req { color: var(--accent); }
  .newname {
    width: 100%; min-width: 0;
    padding: 0.45rem 0.6rem;
    font-family: var(--font-mono); font-size: 0.88rem;
    color: var(--ink);
    background: var(--surface); border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
  }
  .newname:focus { outline: none; border-color: var(--accent); }

  .folderfield { display: block; margin-bottom: 1.4rem; animation: rise 0.4s 0.14s both; }
  .folderpick {
    width: 100%;
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.45rem 0.6rem;
    background: var(--surface); border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm); text-align: left;
    transition: border-color 0.15s;
  }
  .folderpick:hover { border-color: var(--accent); }
  .folderpick-glyph { flex: none; color: var(--k-container); font-family: var(--font-mono); font-size: 0.9rem; }
  .folderpick-text {
    flex: 1; min-width: 0;
    font-family: var(--font-mono); font-size: 0.88rem; color: var(--ink-faint);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .folderpick.chosen .folderpick-text { color: var(--ink); }
  .folderpick-action {
    flex: none; font-family: var(--font-mono); font-size: 0.66rem; letter-spacing: 0.08em;
    text-transform: uppercase; color: var(--accent);
  }
  .fielderror { margin: 0.4rem 0 0; font-size: 0.78rem; color: var(--err, #f87171); line-height: 1.5; }
  .folderpick[aria-invalid="true"] { border-color: var(--err, #f87171); }
  .fieldhint { margin: 0.4rem 0 0; font-size: 0.78rem; color: var(--ink-faint); line-height: 1.5; }
  .fieldhint code { font-family: var(--font-mono); font-size: 0.95em; color: var(--ink-soft); }

  /* The "enter a name first" hint; turns from prompt to instruction once valid. */
  .hint { margin: -0.4rem 0 0.85rem; font-size: 0.8rem; color: var(--ink-faint); line-height: 1.5; }
  .hint.armed { color: var(--ink-soft); }

  .cards { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr)); gap: 0.7rem; animation: rise 0.4s 0.16s both; }
  .card {
    position: relative; width: 100%; height: 100%;
    display: flex; flex-direction: column; gap: 0.5rem;
    text-align: left;
    padding: 1rem 1.05rem 0.9rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    transition: border-color 0.15s, transform 0.15s, background 0.15s;
  }
  .card:hover:not(:disabled) {
    border-color: var(--accent);
    transform: translateY(-2px);
    background: color-mix(in srgb, var(--surface-2) 84%, var(--accent) 8%);
  }
  .card:disabled { opacity: 0.4; cursor: not-allowed; }
  /* card corner ticks (top-left, bottom-right), revealed on hover */
  .ct { position: absolute; width: 9px; height: 9px; border: 1.5px solid var(--accent); opacity: 0; transition: opacity 0.15s; }
  .ct.tl { top: 5px; left: 5px; border-right: 0; border-bottom: 0; }
  .ct.br { bottom: 5px; right: 5px; border-left: 0; border-top: 0; }
  .card:hover:not(:disabled) .ct { opacity: 0.75; }
  .card-top { display: flex; align-items: baseline; justify-content: space-between; gap: 0.6rem; }
  .card-name { font-family: var(--font-display); font-weight: 700; font-size: 1.08rem; letter-spacing: -0.02em; }
  .count { flex: none; font-family: var(--font-mono); font-size: 0.64rem; color: var(--ink-faint); }
  .desc { color: var(--ink-soft); font-size: 0.84rem; line-height: 1.5; }
  .go {
    margin-top: 0.15rem; font-family: var(--font-mono); font-size: 0.68rem; letter-spacing: 0.08em;
    text-transform: uppercase; color: var(--accent); display: inline-flex; align-items: center; gap: 0.4rem;
  }
  .arr { transition: transform 0.15s; }
  .card:hover:not(:disabled) .arr { transform: translateX(3px); }
</style>
