<script lang="ts">
  import { core } from "../lib/core";
  import { app, type Transfer } from "../lib/state.svelte";
  import { bytes, eta, fileName, rate } from "../lib/format";
  import type { TransferState } from "../lib/types";

  let showDone = $state(true);

  const live = (s: TransferState) =>
    s.type === "queued" || s.type === "active" || s.type === "paused";

  const list = $derived.by(() => {
    const all = app.transferList;
    const shown = showDone ? all : all.filter((t) => live(t.state));
    // Live transfers first — they are the ones the user is waiting on.
    return [...shown].sort((a, b) => {
      const byLive = Number(live(b.state)) - Number(live(a.state));
      return byLive !== 0 ? byLive : fileName(a.path).localeCompare(fileName(b.path));
    });
  });

  const done = $derived(app.transferList.filter((t) => !live(t.state)).length);

  function progress(state: TransferState): number {
    switch (state.type) {
      case "active":
        return state.data.total > 0 ? state.data.transferred / state.data.total : 0;
      case "paused":
        return state.data.total > 0 ? state.data.transferred / state.data.total : 0;
      case "completed":
        return 1;
      default:
        return 0;
    }
  }

  /** The right-hand status line for a row. */
  function detail(state: TransferState): string {
    switch (state.type) {
      case "queued":
        return state.data.place === null ? "Queued" : `Queued · position ${state.data.place}`;
      case "active": {
        const { transferred, total, bytesPerSec } = state.data;
        const remaining = eta(transferred, total, bytesPerSec);
        const speed = rate(bytesPerSec);
        return remaining ? `${speed} · ${remaining} left` : speed;
      }
      case "paused":
        return `Paused at ${bytes(state.data.transferred)}`;
      case "completed":
        return "Completed";
      case "failed":
        return state.data.reason ?? "Failed";
      case "cancelled":
        return "Cancelled";
      case "timedOut":
        return "Timed out";
    }
  }

  function tone(state: TransferState): string {
    if (state.type === "completed") return "ok";
    if (state.type === "failed" || state.type === "timedOut") return "danger";
    if (state.type === "cancelled") return "";
    return "";
  }

  async function act(action: () => Promise<void>) {
    try {
      await action();
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  const clearable = $derived(done > 0);
  function clearFinished(t: Transfer) {
    void core.cancelTransfer(t.username, t.path);
  }
</script>

<div class="view">
  <header>
    <h2>Transfers</h2>
    <label class="check">
      <input type="checkbox" bind:checked={showDone} />
      <span>Show finished</span>
    </label>
    <button
      class="btn quiet small"
      disabled={!clearable}
      onclick={() => app.transferList.filter((t) => !live(t.state)).forEach(clearFinished)}
    >
      Clear finished
    </button>
  </header>

  {#if list.length === 0}
    <div class="empty">
      <h3>No transfers</h3>
      <p>Downloads you start from search results appear here.</p>
    </div>
  {:else}
    <div class="body">
      {#each list as t (t.username + t.path)}
        {@const p = progress(t.state)}
        <div class="row">
          <div class="bar" style="--p: {p * 100}%" data-state={t.state.type}></div>

          <div class="info">
            <span class="name selectable" title={t.path}>{fileName(t.path)}</span>
            <span class="from">from {t.username}</span>
          </div>

          <div class="status">
            <span class="detail {tone(t.state)}">{detail(t.state)}</span>
            {#if t.state.type === "active"}
              <span class="num dim">
                {bytes(t.state.data.transferred)} / {bytes(t.state.data.total)}
              </span>
            {/if}
          </div>

          <div class="actions">
            {#if t.state.type === "active" || t.state.type === "queued"}
              <button
                class="btn quiet small"
                onclick={() => act(() => core.pauseTransfer(t.username, t.path))}
              >
                Pause
              </button>
            {:else if t.state.type === "paused"}
              <button
                class="btn quiet small"
                onclick={() => act(() => core.resumeTransfer(t.username, t.path))}
              >
                Resume
              </button>
            {/if}
            {#if live(t.state)}
              <button
                class="btn quiet small danger"
                onclick={() => act(() => core.cancelTransfer(t.username, t.path))}
              >
                Cancel
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 13px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    margin-right: auto;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-2);
    cursor: pointer;
  }
  .check input {
    accent-color: var(--accent);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .row {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 14px;
    padding: 11px 16px;
    border-bottom: 1px solid var(--line-soft);
  }
  .row:hover {
    background: var(--surface-2);
  }

  /* The progress fill is a background layer rather than a separate bar, so a
     busy list reads as a set of filling rows instead of a field of widgets. */
  .bar {
    position: absolute;
    inset: 0;
    width: var(--p);
    background: var(--accent-quiet);
    transition: width var(--slow);
    pointer-events: none;
  }
  .bar[data-state="completed"] {
    background: var(--ok-quiet);
  }
  .bar[data-state="paused"] {
    background: var(--warn-quiet);
  }

  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
    z-index: 1;
  }
  .name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .from {
    font-size: 11.5px;
    color: var(--text-3);
  }

  .status {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.3;
    z-index: 1;
    white-space: nowrap;
  }
  .detail {
    font-size: 12px;
    color: var(--text-2);
  }
  .detail.ok {
    color: var(--ok);
  }
  .detail.danger {
    color: var(--danger);
  }
  .dim {
    font-size: 11px;
    color: var(--text-3);
  }

  .actions {
    display: flex;
    gap: 4px;
    z-index: 1;
  }
  .btn.danger:hover {
    color: var(--danger);
  }
</style>
