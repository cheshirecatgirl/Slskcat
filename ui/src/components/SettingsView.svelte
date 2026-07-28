<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import type { Settings } from "../lib/types";

  const settings = $derived(app.settings);

  /**
   * Apply a change: persist it, and push the parts the live session cares
   * about to the core. Persisting first means a crash before the next launch
   * still leaves the preference saved.
   */
  async function update(change: Partial<Settings>) {
    if (!settings) return;
    const next = { ...settings, ...change };
    try {
      app.settings = await core.saveSettings(next);
    } catch (error) {
      app.notify(`Could not save your settings: ${error}`, "danger");
      return;
    }

    try {
      if (change.sharedDirs) await core.setSharedDirs(next.sharedDirs);
      if (change.uploadSlots !== undefined) await core.setUploadSlots(next.uploadSlots);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  async function addFolders() {
    const picked = await open({ directory: true, multiple: true });
    if (!picked || !settings) return;
    const added = Array.isArray(picked) ? picked : [picked];
    // A folder shared twice would be offered twice; keep the set distinct.
    await update({ sharedDirs: [...new Set([...settings.sharedDirs, ...added])] });
  }

  async function removeFolder(path: string) {
    if (!settings) return;
    await update({ sharedDirs: settings.sharedDirs.filter((p) => p !== path) });
  }

  async function chooseDownloadDir() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") await update({ downloadDir: picked });
  }

  /**
   * Slot changes are saved when the drag ends rather than on every step, so a
   * sweep across the range writes once instead of ten times.
   */
  function onSlots(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    void update({ uploadSlots: value });
  }
</script>

<div class="view">
  <header><h2>Settings</h2></header>

  {#if !settings}
    <div class="empty">
      <h3>Loading your settings…</h3>
    </div>
  {:else}
    <div class="body">
      <section>
        <h3>Download folder</h3>
        <p class="hint">Where finished files are saved.</p>
        <div class="row">
          <span class="path selectable" title={settings.downloadDir}>
            {settings.downloadDir}
          </span>
          <button class="btn small" onclick={chooseDownloadDir}>Change…</button>
        </div>
      </section>

      <section>
        <h3>Shared folders</h3>
        <p class="hint">
          Files in these folders are offered to other users. Sharing nothing is
          allowed, but many users will not queue you if you share nothing.
        </p>

        {#if settings.sharedDirs.length > 0}
          <ul>
            {#each settings.sharedDirs as path (path)}
              <li>
                <span class="path selectable" title={path}>{path}</span>
                <button class="btn quiet small" onclick={() => removeFolder(path)}>Remove</button>
              </li>
            {/each}
          </ul>
        {/if}

        <button class="btn" onclick={addFolders}>Add folder…</button>

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
          <input
            type="range"
            min="1"
            max="10"
            value={settings.uploadSlots}
            onchange={onSlots}
          />
          <span class="num value">{settings.uploadSlots}</span>
        </div>
      </section>

      <section>
        <h3>Account</h3>
        <p class="hint">
          Signed in as <strong>{app.username}</strong>.
          {#if settings.rememberPassword}
            Your password is saved in this system's credential store.
          {:else}
            Your password is not saved.
          {/if}
        </p>
        <div class="row">
          {#if settings.rememberPassword}
            <button class="btn" onclick={() => update({ rememberPassword: false, password: "" })}>
              Forget my password
            </button>
          {/if}
          <button class="btn" onclick={() => core.disconnect()}>Sign out</button>
        </div>
      </section>
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

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
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
