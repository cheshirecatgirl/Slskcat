<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";

  let shared = $state<string[]>([]);
  let slots = $state(2);

  async function addFolder() {
    const picked = await open({ directory: true, multiple: true });
    if (!picked) return;
    const added = Array.isArray(picked) ? picked : [picked];
    // A folder shared twice would be uploaded twice; keep the set distinct.
    shared = [...new Set([...shared, ...added])];
    await push();
  }

  async function removeFolder(path: string) {
    shared = shared.filter((p) => p !== path);
    await push();
  }

  async function push() {
    try {
      await core.setSharedDirs($state.snapshot(shared));
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  async function applySlots() {
    try {
      await core.setUploadSlots(slots);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }
</script>

<div class="view">
  <header><h2>Settings</h2></header>

  <div class="body">
    <section>
      <h3>Shared folders</h3>
      <p class="hint">
        Files in these folders are offered to other users. Sharing nothing is
        allowed, but many users will not queue you if you share nothing.
      </p>

      {#if shared.length > 0}
        <ul>
          {#each shared as path (path)}
            <li>
              <span class="path selectable" title={path}>{path}</span>
              <button class="btn quiet small" onclick={() => removeFolder(path)}>Remove</button>
            </li>
          {/each}
        </ul>
      {/if}

      <button class="btn" onclick={addFolder}>Add folder…</button>

      {#if app.shares}
        <p class="stat num">
          Currently sharing {app.shares.files.toLocaleString()} files across
          {app.shares.directories.toLocaleString()} folders.
        </p>
      {/if}
    </section>

    <section>
      <h3>Upload slots</h3>
      <p class="hint">
        How many people can download from you at once. Everyone else waits in
        your queue.
      </p>
      <div class="slots">
        <input type="range" min="1" max="10" bind:value={slots} onchange={applySlots} />
        <span class="num value">{slots}</span>
      </div>
    </section>

    <section>
      <h3>Account</h3>
      <p class="hint">Signed in as <strong>{app.username}</strong>.</p>
      <button class="btn" onclick={() => core.disconnect()}>Sign out</button>
    </section>
  </div>
</div>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  header {
    padding: 15px 18px 4px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px 18px 40px;
  }

  section {
    max-width: 560px;
    padding: 16px 18px;
    margin-bottom: 10px;
    border-radius: var(--radius-lg);
    background: var(--surface-2);
  }

  h3 {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 4px;
  }
  .hint {
    color: var(--text-3);
    font-size: 12.5px;
    line-height: 1.55;
    margin-bottom: 11px;
  }
  .hint strong {
    color: var(--text-2);
  }

  ul {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 9px;
    border-radius: var(--radius-sm);
    background: var(--surface);
  }
  .path {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .stat {
    margin-top: 10px;
    font-size: 12px;
    color: var(--text-3);
  }

  .slots {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .slots input {
    flex: 1;
    max-width: 260px;
    accent-color: var(--accent);
  }
  .value {
    font-size: 13px;
    font-weight: 600;
    min-width: 2ch;
  }
</style>
