<script lang="ts">
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { bytes, fileName, rate, tailPath } from "../lib/format";

  let who = $state("");
  let pending = $state(false);
  let open = $state<Record<string, boolean>>({});

  /**
   * The box filters the people already seen rather than gating on an exact
   * name. Browsing used to require knowing who to ask before anything at all
   * appeared, which is a barrier in front of a list the session already had.
   */
  const listed = $derived.by(() => {
    const needle = who.trim().toLowerCase();
    const kept = new Set(app.settings?.friends ?? []);
    const known = app.knownUsers;
    const ordered = [...known].sort((a, b) => {
      // Friends first: they are the reason to keep a list at all.
      const byFriend = Number(kept.has(b)) - Number(kept.has(a));
      return byFriend !== 0 ? byFriend : a.localeCompare(b);
    });
    const matched = needle
      ? ordered.filter((name) => name.toLowerCase().includes(needle))
      : ordered;
    return matched.slice(0, 400);
  });

  /** A name typed that nobody has been seen under is still worth asking. */
  const unlisted = $derived.by(() => {
    const typed = who.trim();
    if (!typed) return null;
    return listed.some((name) => name.toLowerCase() === typed.toLowerCase()) ? null : typed;
  });

  async function browse(name: string) {
    // Deliberately does not fill the box. The box is the filter; writing the
    // selection into it collapses the list to the one name just picked, which
    // is the barrier this list exists to remove.
    pending = true;
    app.browseResult = null;
    try {
      await core.browseUser(name);
      await core.requestUserInfo(name);
    } catch (error) {
      app.notify(String(error), "danger");
      pending = false;
    }
  }

  const directories = $derived(app.browseResult ?? []);
  const info = $derived(app.browsing ? app.users[app.browsing] : undefined);
  const totals = $derived.by(() => {
    let files = 0;
    let size = 0;
    for (const dir of directories) {
      files += dir.files.length;
      for (const file of dir.files) size += file.size;
    }
    return { files, size };
  });

  // Asking the server about them at the same time is deliberate: whether they
  // are online and how fast they are decides whether the listing is worth
  // acting on.
  function submit(event: SubmitEvent) {
    event.preventDefault();
    const name = who.trim();
    if (name) void browse(name);
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
  <aside>
    <div class="pane-head">
      <form onsubmit={submit}>
        <input
          class="field slim"
          bind:value={who}
          placeholder="Find a user…"
          spellcheck="false"
          autocapitalize="off"
        />
      </form>
    </div>

    {#if unlisted}
      <button class="entry unlisted" onclick={() => browse(unlisted ?? "")}>
        Browse <strong>{unlisted}</strong>
      </button>
    {/if}

    {#each listed as person (person)}
      <button class="entry" class:active={app.browsing === person} onclick={() => browse(person)}>
        <span class="name">{person}</span>
        {#if (app.settings?.friends ?? []).includes(person)}
          <span class="star" aria-hidden="true">★</span>
        {/if}
      </button>
    {:else}
      <p class="none">
        {app.knownUsers.length === 0 ? "Nobody seen yet." : "No matches."}
      </p>
    {/each}
  </aside>

  <section>
    <header>
    {#if app.browsing && app.browseResult}
      <p class="meta">
        <strong>{app.browsing}</strong>
        {#if info?.presence}
          <span class="tag {info.presence === 'online' ? 'ok' : ''}">{info.presence}</span>
        {/if}
        <span class="num">
          {directories.length.toLocaleString()} folders ·
          {totals.files.toLocaleString()} files · {bytes(totals.size)}
        </span>
        {#if info?.averageSpeed}
          <span class="num">avg {rate(info.averageSpeed)}</span>
        {/if}
        {#if info?.sharedFiles}
          <span class="num">shares {info.sharedFiles.toLocaleString()}</span>
        {/if}
      </p>
    {/if}
    </header>

  {#if pending}
    <div class="empty">
      <h3>Waiting for {who}</h3>
    </div>
  {:else if !app.browseResult}
    <div class="empty">
      <h3>No user browsed</h3>
    </div>
  {:else if directories.length === 0}
    <div class="empty">
      <h3>Nothing shared</h3>
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
  </section>
</div>

<style>
  .view {
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    height: 100%;
  }
  /* The same shape as the messages pane, for the same reason: a list of names
     beside the thing a name opens. */
  aside {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    background: var(--surface-2);
  }
  .pane-head {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 11px 10px;
    background: var(--surface-2);
  }
  /* The panel is `--surface-2`, which is what `.field` uses, so an unmodified
     input would be invisible against it. */
  .pane-head .field.slim {
    padding: 5px 9px;
    font-size: 12.5px;
    background: var(--surface);
    border-color: var(--line-soft);
  }
  .entry {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 6px 1px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background var(--fast);
  }
  .entry:hover {
    background: var(--surface-3);
  }
  .entry.active {
    background: var(--accent-quiet);
    color: var(--accent);
  }
  .entry .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .entry .star {
    flex: none;
    font-size: 10px;
    color: var(--accent);
  }
  .unlisted {
    background: var(--accent-quiet);
    color: var(--text-2);
    font-size: 12.5px;
  }
  .unlisted strong {
    color: var(--text-1);
    font-weight: 600;
  }
  .none {
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-3);
  }

  section {
    display: flex;
    flex-direction: column;
    min-width: 0;
    height: 100%;
  }
  header {
    padding: 15px 18px 12px;
  }
  form {
    display: flex;
    gap: 8px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
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
