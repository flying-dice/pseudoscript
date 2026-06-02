<script lang="ts">
  import { X } from "@lucide/svelte";

  import ProblemsPane from "../ProblemsPane.svelte";

  type Problem = { severity: string; message: string; file?: string; start_line: number; start_col: number; code?: string };

  type Props = {
    problems?: Problem[];
    onpick?: (d: Problem) => void;
    oncopy?: (text: string, count: number) => void;
    oncollapse?: () => void;
  };

  let { problems = [], onpick, oncopy, oncollapse }: Props = $props();
</script>

<section class="dock island">
  <header class="panel-head">
    <span class="title">Problems</span>
    <span class="count">{problems.length}</span>
    <button class="collapse" onclick={oncollapse} aria-label="Hide problems" title="Hide problems">
      <X size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>
  </header>
  <div class="panel-body">
    <ProblemsPane diagnostics={problems} {onpick} {oncopy} />
  </div>
</section>

<style>
  .dock {
    display: grid;
    grid-template-rows: var(--bar-h, 34px) 1fr;
    min-height: 0;
    background: var(--island-bg);
  }
  .panel-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: var(--bar-h, 34px);
    padding: 0 0.4rem 0 0.7rem;
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    color: var(--ink-faint);
  }
  .collapse {
    margin-left: auto;
    width: 1.5rem;
    height: 1.5rem;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--ink-faint);
    cursor: pointer;
  }
  .collapse:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .panel-body {
    min-height: 0;
    overflow: auto;
  }
</style>
