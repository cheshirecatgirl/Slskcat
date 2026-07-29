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
      app.notify(`Could not save settings: ${error}`, "danger");
      return;
    }

    try {
      if (change.sharedDirs) await core.setSharedDirs(next.sharedDirs);
      if (change.uploadSlots !== undefined) await core.setUploadSlots(next.uploadSlots);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  /** Folders the core flagged as personal, awaiting an explicit yes. */
  let flagged = $state<{ path: string; reason: string }[]>([]);

  /**
   * Add folders, checking each one first.
   *
   * The core refuses dangerous paths regardless, but asking here means a
   * refusal is explained while the user still has the folder in mind, and a
   * merely personal folder can be confirmed rather than silently accepted.
   */
  async function addFolders() {
    const picked = await open({ directory: true, multiple: true });
    if (!picked || !settings) return;

    const safe: string[] = [];
    for (const path of Array.isArray(picked) ? picked : [picked]) {
      const verdict = await core.assessShare(path);
      if (!verdict.allowed) {
        app.notify(`Not shared: ${verdict.reason}`, "danger");
      } else if (verdict.sensitive) {
        flagged = [...flagged, { path, reason: verdict.reason ?? "" }];
      } else {
        safe.push(path);
      }
    }
    if (safe.length > 0) await share(safe);
  }

  /** Add paths to the share set, keeping it distinct. */
  async function share(paths: string[]) {
    if (!settings) return;
    await update({ sharedDirs: [...new Set([...settings.sharedDirs, ...paths])] });
  }

  async function confirmFlagged(path: string) {
    flagged = flagged.filter((f) => f.path !== path);
    await share([path]);
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
      <h3>Loading settings…</h3>
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
        <p class="hint">Offered to other users.</p>

        {#each flagged as item (item.path)}
          <div class="flagged" role="alert">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 2 1 21h22L12 2Zm0 4.5L19.5 19h-15L12 6.5ZM11 10v5h2v-5h-2Zm0 6v2h2v-2h-2Z" />
            </svg>
            <div class="flagged-text">
              <span class="path selectable" title={item.path}>{item.path}</span>
              <span class="why">{item.reason}</span>
            </div>
            <button class="btn small" onclick={() => confirmFlagged(item.path)}>Share anyway</button>
            <button
              class="btn quiet small"
              onclick={() => (flagged = flagged.filter((f) => f.path !== item.path))}>Skip</button
            >
          </div>
        {/each}

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
            Sharing {app.shares.files.toLocaleString()} files in
            {app.shares.directories.toLocaleString()} folders.
          </p>
        {/if}
      </section>

      <section>
        <h3>Upload slots</h3>
        <p class="hint">Concurrent uploads. Others wait in the queue.</p>
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
          {settings.rememberPassword
            ? "Password stored in the system keychain."
            : "Password not stored."}
        </p>
        <div class="row">
          {#if settings.rememberPassword}
            <button class="btn" onclick={() => update({ rememberPassword: false, password: "" })}>
              Forget password
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

  .flagged {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
    padding: 9px 11px;
    border-radius: var(--radius-sm);
    background: var(--warn-quiet);
    animation: flag var(--spring) both;
  }
  @keyframes flag {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .flagged svg {
    width: 16px;
    height: 16px;
    flex: none;
    fill: var(--warn);
  }
  .flagged-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    line-height: 1.35;
  }
  .why {
    font-size: 11.5px;
    color: var(--warn);
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
