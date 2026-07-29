<script lang="ts">
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { bytes, eta, fileName, rate } from "../lib/format";
  import type { TransferState, UploadState } from "../lib/types";

  let tab = $state<"down" | "up">("down");
  let showDone = $state(true);

  const liveDown = (s: TransferState) =>
    s.type === "queued" || s.type === "active" || s.type === "paused";
  const liveUp = (s: UploadState) => s.type === "queued" || s.type === "active";

  const downloads = $derived.by(() => {
    const all = app.transferList;
    const shown = showDone ? all : all.filter((t) => liveDown(t.state));
    // In-flight first: those are the ones being waited on.
    return [...shown].sort((a, b) => {
      const byLive = Number(liveDown(b.state)) - Number(liveDown(a.state));
      return byLive !== 0 ? byLive : fileName(a.path).localeCompare(fileName(b.path));
    });
  });

  const uploads = $derived.by(() => {
    const all = app.uploadList;
    const shown = showDone ? all : all.filter((u) => liveUp(u.state));
    return [...shown].sort((a, b) => {
      const byLive = Number(liveUp(b.state)) - Number(liveUp(a.state));
      return byLive !== 0 ? byLive : fileName(a.path).localeCompare(fileName(b.path));
    });
  });

  const finished = $derived(
    app.transferList.filter((t) => !liveDown(t.state)).length +
      app.uploadList.filter((u) => !liveUp(u.state)).length,
  );

  const ratio = (done: number, total: number) => (total > 0 ? Math.min(done / total, 1) : 0);

  /** Fill fraction for a download row's background, 0–1. */
  function downProgress(state: TransferState): number {
    switch (state.type) {
      case "active":
      case "paused":
        return ratio(state.data.transferred, state.data.total);
      case "completed":
        return 1;
      default:
        return 0;
    }
  }

  /** The same for an upload, whose byte counts live on the row, not the state. */
  function upProgress(state: UploadState, sent: number, size: number): number {
    if (state.type === "completed") return 1;
    return state.type === "active" ? ratio(sent, size) : 0;
  }

  function downDetail(state: TransferState): string {
    switch (state.type) {
      case "queued":
        return state.data.place === null ? "Queued" : `Queued · position ${state.data.place}`;
      case "active": {
        const { transferred, total, bytesPerSec } = state.data;
        const left = eta(transferred, total, bytesPerSec);
        return left ? `${rate(bytesPerSec)} · ${left} left` : rate(bytesPerSec);
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

  function upDetail(state: UploadState, bytesPerSec: number): string {
    switch (state.type) {
      case "queued":
        return `Queued · position ${state.data.place}`;
      case "active":
        return rate(bytesPerSec);
      case "completed":
        return "Sent";
      case "cancelled":
        return "Cancelled";
      case "failed":
        return state.data.reason;
    }
  }

  function tone(type: string): string {
    if (type === "completed") return "ok";
    if (type === "failed" || type === "timedOut") return "danger";
    return "";
  }

  async function act(action: () => Promise<void>) {
    try {
      await action();
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  function clearFinished() {
    for (const t of app.transferList.filter((t) => !liveDown(t.state))) {
      void core.cancelTransfer(t.username, t.path);
    }
  }
</script>

<div class="view">
  <header>
    <div class="tabs" role="tablist">
      <button
        class="tab"
        class:on={tab === "down"}
        role="tab"
        aria-selected={tab === "down"}
        onclick={() => (tab = "down")}
      >
        Downloads
        {#if app.activeTransfers > 0}<span class="pip num">{app.activeTransfers}</span>{/if}
      </button>
      <button
        class="tab"
        class:on={tab === "up"}
        role="tab"
        aria-selected={tab === "up"}
        onclick={() => (tab = "up")}
      >
        Uploads
        {#if app.activeUploads > 0}<span class="pip num">{app.activeUploads}</span>{/if}
      </button>
    </div>

    <label class="check">
      <input type="checkbox" bind:checked={showDone} />
      <span>Show finished</span>
    </label>
    {#if tab === "down"}
      <button class="btn quiet small" disabled={finished === 0} onclick={clearFinished}>
        Clear finished
      </button>
    {/if}
  </header>

  {#if tab === "down"}
    {#if downloads.length === 0}
      <div class="empty"><h3>No downloads</h3></div>
    {:else}
      <div class="body">
        {#each downloads as t (t.username + t.path)}
          <div class="row">
            <div
              class="bar"
              style="--p: {downProgress(t.state) * 100}%"
              data-state={t.state.type}
            ></div>

            <div class="info">
              <span class="name selectable" title={t.path}>{fileName(t.path)}</span>
              <span class="peer">from {t.username}</span>
            </div>

            <div class="status">
              <span class="detail {tone(t.state.type)}">{downDetail(t.state)}</span>
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
                  onclick={() => act(() => core.pauseTransfer(t.username, t.path))}>Pause</button
                >
              {:else if t.state.type === "paused"}
                <button
                  class="btn quiet small"
                  onclick={() => act(() => core.resumeTransfer(t.username, t.path))}>Resume</button
                >
              {/if}
              {#if liveDown(t.state)}
                <button
                  class="btn quiet small danger"
                  onclick={() => act(() => core.cancelTransfer(t.username, t.path))}>Cancel</button
                >
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {:else if uploads.length === 0}
    <div class="empty"><h3>No uploads</h3></div>
  {:else}
    <div class="body">
      {#each uploads as u (u.username + u.path)}
        <div class="row">
          <div
            class="bar up"
            style="--p: {upProgress(u.state, u.sent, u.size) * 100}%"
            data-state={u.state.type}
          ></div>

          <div class="info">
            <span class="name selectable" title={u.path}>{fileName(u.path)}</span>
            <span class="peer">to {u.username}</span>
          </div>

          <div class="status">
            <span class="detail {tone(u.state.type)}">{upDetail(u.state, u.bytesPerSec)}</span>
            {#if u.state.type === "active"}
              <span class="num dim">{bytes(u.sent)} / {bytes(u.size)}</span>
            {/if}
          </div>

          <div class="actions">
            {#if liveUp(u.state)}
              <button
                class="btn quiet small danger"
                onclick={() => act(() => core.cancelUpload(u.username, u.path))}>Stop</button
              >
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
    padding: 13px 18px 11px;
  }

  .tabs {
    display: flex;
    gap: 2px;
    margin-right: auto;
    padding: 2px;
    border-radius: 999px;
    background: var(--surface-2);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 13px;
    border-radius: 999px;
    color: var(--text-3);
    font-size: 12.5px;
    font-weight: 500;
    transition: background var(--fast), color var(--fast), transform var(--fast);
  }
  .tab:hover:not(.on) {
    color: var(--text-2);
  }
  .tab:active {
    transform: scale(0.97);
  }
  .tab.on {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px #00000033;
  }
  .pip {
    min-width: 17px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 10.5px;
    font-weight: 600;
    text-align: center;
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
    padding-bottom: 10px;
  }

  .row {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 14px;
    margin: 0 10px 3px;
    padding: 11px 14px;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--surface-2);
    transition: background var(--fast);
  }
  .row:hover {
    background: var(--surface-3);
  }

  /* The progress fill is the row's own background rather than a separate
     widget, so a busy list reads as a set of filling rows. */
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
  /* Uploads fill from the right, so direction is legible at a glance even
     before reading the row. */
  .bar.up {
    left: auto;
    right: 0;
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
  .peer {
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
