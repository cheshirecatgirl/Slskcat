<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import * as session from "../lib/session";
  import { blankProxy, type Proxy, type Settings } from "../lib/types";

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
      if (change.downloadSlots !== undefined) await core.setDownloadSlots(next.downloadSlots);

      // A proxy decides where the connection goes, and an open socket cannot
      // be moved onto a different route after the fact. So the session is
      // rebuilt: signing in again is the only thing "apply now" can mean, and
      // doing it here is better than leaving it for the user to discover.
      // The wait rather than the form: there is nothing to fill in, and the
      // old session drops partway through.
      if (change.proxy !== undefined && app.connected) {
        await session.signIn(next, "reconnecting");
      }
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

  /**
   * The scale being dragged towards, before it is applied.
   *
   * Applying it live made the control unusable: zooming the page moves and
   * resizes the slider under the pointer that is dragging it, so the handle
   * runs away from the cursor. The number beside it follows the drag instead,
   * and the page changes once, when the drag ends.
   */
  let preview = $state<number | null>(null);

  $effect(() => {
    // The stored value winning again is how the preview ends: once the change
    // has been written there is nothing left to preview.
    void settings?.uiScale;
    preview = null;
  });

  function onDownloadSlots(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    void update({ downloadSlots: value });
  }

  /** The proxy being edited, or a blank one while the section is switched on. */
  const proxy = $derived(settings?.proxy ?? null);

  /**
   * Changes are written when a field is left rather than on every keystroke,
   * so a half-typed host is never saved and never connected through.
   */
  function editProxy(change: Partial<Proxy>) {
    if (!settings) return;
    void update({ proxy: { ...(settings.proxy ?? blankProxy()), ...change } });
  }

  function toggleProxy(on: boolean) {
    void update({ proxy: on ? blankProxy() : null });
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
        <h3>Download slots</h3>
        <p class="hint">
          Files fetched at once. The rest wait their turn, and a peer that has
          queued you holds its slot until it starts sending.
        </p>
        <div class="slots">
          <input
            type="range"
            min="1"
            max="10"
            value={settings.downloadSlots}
            onchange={onDownloadSlots}
          />
          <span class="num value">{settings.downloadSlots}</span>
        </div>
      </section>

      <section>
        <h3>Wishlist</h3>
        <p class="hint">
          A wish is a search the server keeps re-running. It can queue what it
          finds without asking, up to twenty files per wish per session — a
          loose wish matches a great deal, and the rest stay in the list to
          pick from by hand.
        </p>
        <label class="check">
          <input
            type="checkbox"
            checked={settings.autoDownloadWishes}
            onchange={(event) =>
              void update({ autoDownloadWishes: event.currentTarget.checked })}
          />
          <span>Download what my wishes find</span>
        </label>
      </section>

      <section>
        <h3>Interface scale</h3>
        <p class="hint">Everything larger or smaller together, in steps of ten.</p>
        <div class="slots">
          <input
            type="range"
            min="50"
            max="200"
            step="10"
            value={settings.uiScale}
            oninput={(event) => (preview = Number(event.currentTarget.value))}
            onchange={(event) => update({ uiScale: Number(event.currentTarget.value) })}
          />
          <span class="num value">{preview ?? settings.uiScale}%</span>
        </div>
      </section>

      <section>
        <h3>Proxy</h3>
        <p class="hint">
          Sends the server connection and every peer connection through a proxy.
          Nothing can reach you through one, so listening is switched off while
          it is on — peers cannot open a connection to you, and instead ask the
          server to have you open one to them. Downloads and uploads both still
          work that way; it costs a round trip at the start of each transfer,
          and a peer that never falls back stays out of reach.
        </p>

        <label class="check">
          <input
            type="checkbox"
            checked={proxy !== null}
            onchange={(event) => toggleProxy(event.currentTarget.checked)}
          />
          <span>Connect through a proxy</span>
        </label>

        {#if proxy}
          <div class="proxy">
            <select
              class="field slim auto"
              value={proxy.kind}
              onchange={(event) =>
                editProxy({ kind: event.currentTarget.value as Proxy["kind"] })}
            >
              <option value="socks5">SOCKS5</option>
              <option value="socks4">SOCKS4a</option>
              <option value="http">HTTP</option>
            </select>
            <input
              class="field slim"
              value={proxy.host}
              placeholder="Host"
              spellcheck="false"
              autocapitalize="off"
              onchange={(event) => editProxy({ host: event.currentTarget.value.trim() })}
            />
            <input
              class="field slim port num"
              value={proxy.port}
              type="number"
              min="1"
              max="65535"
              placeholder="Port"
              onchange={(event) => editProxy({ port: Number(event.currentTarget.value) })}
            />
          </div>
          <div class="proxy">
            <input
              class="field slim"
              value={proxy.username}
              placeholder="Username (optional)"
              spellcheck="false"
              autocapitalize="off"
              autocomplete="off"
              onchange={(event) => editProxy({ username: event.currentTarget.value })}
            />
            <input
              class="field slim"
              type="password"
              value={proxy.password}
              placeholder="Password (optional)"
              autocomplete="off"
              onchange={(event) => editProxy({ password: event.currentTarget.value })}
            />
          </div>
          <p class="hint quiet">
            The destination is handed to the proxy as a name, never resolved
            here, so no lookup announces where you are going. Changing this
            signs in again, because a connection already open cannot be moved
            onto a different route.
          </p>
        {/if}
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
  .proxy {
    display: flex;
    gap: 8px;
    margin-top: 9px;
  }
  /* `.field` is `width: 100%`, which in a flex row makes the select wide
     enough to crush the field beside it. */
  .proxy .field {
    flex: 1;
    width: auto;
    min-width: 0;
  }
  .proxy .field.auto {
    flex: none;
  }
  .proxy .port {
    flex: none;
    width: 92px;
  }
  .hint.quiet {
    margin-top: 9px;
    color: var(--text-3);
  }

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
  }
  .value {
    font-size: 13px;
    font-weight: 600;
    min-width: 2ch;
  }
</style>
