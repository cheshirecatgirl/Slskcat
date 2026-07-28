<script lang="ts">
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { bytes, fileName, tailPath } from "../lib/format";

  let who = $state("");
  let pending = $state(false);
  let open = $state<Record<string, boolean>>({});

  const directories = $derived(app.browseResult ?? []);
  const totals = $derived.by(() => {
    let files = 0;
    let size = 0;
    for (const dir of directories) {
      files += dir.files.length;
      for (const file of dir.files) size += file.size;
    }
    return { files, size };
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const name = who.trim();
    if (!name) return;
    pending = true;
    app.browseResult = null;
    try {
      await core.browseUser(name);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  // A listing arriving is what ends the pending state; the request itself only
  // asks the peer, and the reply comes back as an event.
  $effect(() => {
    if (app.browseResult !== null) pending = false;
  });

  async function get(username: string, path: string, size: number) {
    try {
      await core.download(username, path, size);
      app.notify(`Queued ${fileName(path)}`);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  async function getAll(path: string) {
    const dir = directories.find((d) => d.path === path);
    if (!dir || !app.browsing) return;
    for (const file of dir.files) {
      await get(app.browsing, file.path, file.size);
    }
  }
</script>

<div class="view">
  <header>
    <form onsubmit={submit}>
      <input
        class="field"
        bind:value={who}
        placeholder="Browse a user's shares…"
        spellcheck="false"
        autocapitalize="off"
      />
      <button class="btn primary" type="submit" disabled={!who.trim() || pending}>
        {pending ? "Asking…" : "Browse"}
      </button>
    </form>
    {#if app.browsing && app.browseResult}
      <p class="meta">
        <strong>{app.browsing}</strong>
        <span class="num">
          {directories.length.toLocaleString()} folders ·
          {totals.files.toLocaleString()} files · {bytes(totals.size)}
        </span>
      </p>
    {/if}
  </header>

  {#if pending}
    <div class="empty">
      <h3>Waiting for {who}</h3>
      <p>Peers answer browse requests at their own pace, and some never do.</p>
    </div>
  {:else if !app.browseResult}
    <div class="empty">
      <h3>Browse someone's collection</h3>
      <p>Enter a username to see everything they share, folder by folder.</p>
    </div>
  {:else if directories.length === 0}
    <div class="empty">
      <h3>Nothing shared</h3>
      <p>{app.browsing} is not sharing any files.</p>
    </div>
  {:else}
    <div class="body">
      {#each directories as dir (dir.path)}
        <div class="folder">
          <button class="dirline" onclick={() => (open = { ...open, [dir.path]: !open[dir.path] })}>
            <span class="chev" class:open={open[dir.path]} aria-hidden="true">▶</span>
            <span class="dirname selectable" title={dir.path}>{tailPath(dir.path, 3)}</span>
            <span class="dircount num">{dir.files.length}</span>
            <span
              class="btn quiet small"
              role="button"
              tabindex="0"
              onclick={(e) => {
                e.stopPropagation();
                void getAll(dir.path);
              }}
              onkeydown={(e) => e.key === "Enter" && getAll(dir.path)}
            >
              Get all
            </span>
          </button>

          {#if open[dir.path]}
            <div class="files">
              {#each dir.files as file (file.path)}
                <div class="file">
                  <span class="fname selectable">{fileName(file.path)}</span>
                  <span class="fsize num">{bytes(file.size)}</span>
                  <button
                    class="btn small"
                    onclick={() => app.browsing && get(app.browsing, file.path, file.size)}
                  >
                    Get
                  </button>
                </div>
              {/each}
            </div>
          {/if}
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
    padding: 15px 18px 12px;
  }
  form {
    display: flex;
    gap: 8px;
  }
  form .btn {
    flex: none;
  }
  .meta {
    display: flex;
    gap: 10px;
    align-items: baseline;
    margin-top: 9px;
    font-size: 12px;
    color: var(--text-3);
  }
  .meta strong {
    color: var(--text);
    font-weight: 600;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .dirline {
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 8px 18px;
    text-align: left;
    transition: background var(--fast);
  }
  .dirline:hover {
    background: var(--accent-quiet);
  }
  .chev {
    font-size: 8px;
    color: var(--text-3);
    transition: transform var(--fast);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .dirname {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dircount {
    font-size: 11px;
    color: var(--text-3);
  }

  .files {
    margin: 2px 10px 6px;
    border-radius: var(--radius);
    background: var(--surface-2);
    overflow: hidden;
  }
  .file {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 82px 52px;
    align-items: center;
    gap: 12px;
    padding: 5px 14px 5px 30px;
  }
  .file:hover {
    background: var(--surface-2);
  }
  .fname {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fsize {
    font-size: 11.5px;
    color: var(--text-3);
    text-align: right;
  }
</style>
