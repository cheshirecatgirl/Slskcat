<script lang="ts">
  import { app } from "../lib/state.svelte";
</script>

<div class="stack" role="status" aria-live="polite">
  {#each app.notices as notice (notice.id)}
    <div class="notice {notice.tone}">
      <span class="text">{notice.text}</span>
      <button class="close" onclick={() => app.dismiss(notice.id)} aria-label="Dismiss">×</button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 50;
    display: flex;
    flex-direction: column;
    gap: 7px;
    /* The stack must not swallow clicks on whatever is behind it. */
    pointer-events: none;
  }

  .notice {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    max-width: 380px;
    padding: 9px 11px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--surface-3);
    box-shadow: var(--shadow);
    pointer-events: auto;
    animation: rise var(--slow) both;
  }
  .notice.danger {
    border-color: color-mix(in srgb, var(--danger) 40%, var(--line));
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  .text {
    flex: 1;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--text);
    overflow-wrap: anywhere;
  }
  .notice.danger .text {
    color: var(--danger);
  }

  .close {
    flex: none;
    padding: 0 3px;
    font-size: 15px;
    line-height: 1;
    color: var(--text-3);
  }
  .close:hover {
    color: var(--text);
  }
</style>
