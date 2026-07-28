<script lang="ts">
  import { core } from "../lib/core";
  import { app, type ResultRow } from "../lib/state.svelte";
  import { bitrate, bytes, duration, extension, fileName, parentPath, rate, tailPath } from "../lib/format";

  let { onCommand }: { onCommand: () => void } = $props();

  let query = $state("");
  let filter = $state("");
  let readyOnly = $state(false);
  let format = $state("any");
  let sortKey = $state<"name" | "size" | "bitrate" | "speed" | "user">("speed");
  let sortAsc = $state(false);

  /** Row height in px. Fixed, which is what makes windowing cheap and exact. */
  const ROW = 34;
  /** Rows rendered beyond the viewport, so fast scrolling never shows gaps. */
  const OVERSCAN = 6;

  let viewport = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const search = $derived(app.search);

  const formats = $derived.by(() => {
    const seen = new Set<string>();
    for (const row of search?.rows ?? []) {
      const ext = extension(row.path);
      if (ext) seen.add(ext);
    }
    return [...seen].sort();
  });

  const rows = $derived.by(() => {
    let list = search?.rows ?? [];

    const needle = filter.trim().toLowerCase();
    if (needle) list = list.filter((r) => r.path.toLowerCase().includes(needle));
    if (readyOnly) list = list.filter((r) => r.freeSlots > 0);
    if (format !== "any") list = list.filter((r) => extension(r.path) === format);

    const direction = sortAsc ? 1 : -1;
    // Copy before sorting: the source array is shared state.
    return [...list].sort((a, b) => {
      switch (sortKey) {
        case "name":
          return direction * fileName(a.path).localeCompare(fileName(b.path));
        case "size":
          return direction * (a.size - b.size);
        case "bitrate":
          return direction * ((a.bitrate ?? 0) - (b.bitrate ?? 0));
        case "user":
          return direction * a.username.localeCompare(b.username);
        default:
          return direction * (a.speed - b.speed);
      }
    });
  });

  // The rendered window: only these rows exist in the DOM at any moment.
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const count = $derived(Math.ceil(viewportHeight / ROW) + OVERSCAN * 2);
  const visible = $derived(rows.slice(first, first + count));

  function measure() {
    if (!viewport) return;
    scrollTop = viewport.scrollTop;
    viewportHeight = viewport.clientHeight;
  }

  function sortBy(key: typeof sortKey) {
    if (sortKey === key) {
      sortAsc = !sortAsc;
    } else {
      sortKey = key;
      // Names read best A–Z; every numeric column reads best largest-first.
      sortAsc = key === "name" || key === "user";
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const text = query.trim();
    if (!text) return;
    try {
      const id = await core.search(text);
      app.startSearch(id, text);
      filter = "";
      format = "any";
      if (viewport) viewport.scrollTop = 0;
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  async function download(row: ResultRow) {
    try {
      await core.download(row.username, row.path, row.size);
      app.notify(`Queued ${fileName(row.path)}`);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  const columns: { key: typeof sortKey; label: string; cls: string }[] = [
    { key: "name", label: "Name", cls: "c-name" },
    { key: "size", label: "Size", cls: "c-size" },
    { key: "bitrate", label: "Bitrate", cls: "c-rate" },
    { key: "user", label: "User", cls: "c-user" },
    { key: "speed", label: "Speed", cls: "c-speed" },
  ];
</script>

<div class="view">
  <header>
    <form onsubmit={submit}>
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M11 4a7 7 0 1 0 4.2 12.6l3.6 3.6 1.4-1.4-3.6-3.6A7 7 0 0 0 11 4Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z"
        />
      </svg>
      <input
        class="field"
        bind:value={query}
        placeholder="Search the network…"
        spellcheck="false"
        autocapitalize="off"
      />
      <button class="btn primary" type="submit" disabled={!query.trim()}>Search</button>
      <button class="btn quiet" type="button" onclick={onCommand} title="Command bar">
        <span class="kbd">⌘K</span>
      </button>
    </form>

    {#if app.searches.length > 0}
      <div class="tabs">
        {#each app.searches.slice(0, 8) as item (item.id)}
          <button
            class="tab"
            class:active={app.activeSearch === item.id}
            onclick={() => (app.activeSearch = item.id)}
          >
            {#if item.running}<span class="pulse" aria-hidden="true"></span>{/if}
            <span class="label">{item.query}</span>
            <span class="count num">{item.rows.length}</span>
          </button>
        {/each}
      </div>
    {/if}
  </header>

  {#if !search}
    <div class="empty">
      <h3>No search yet</h3>
    </div>
  {:else}
    <div class="toolbar">
      <input class="field slim" bind:value={filter} placeholder="Filter these results…" />
      <select class="field slim auto" bind:value={format}>
        <option value="any">Any format</option>
        {#each formats as ext (ext)}
          <option value={ext}>{ext.toUpperCase()}</option>
        {/each}
      </select>
      <label class="check">
        <input type="checkbox" bind:checked={readyOnly} />
        <span>Free slots only</span>
      </label>
      <span class="summary num">
        {rows.length.toLocaleString()} of {search.rows.length.toLocaleString()}
        {#if search.running}<span class="running">· searching…</span>{/if}
      </span>
    </div>

    <div class="head">
      {#each columns as col (col.key)}
        <button class="th {col.cls}" onclick={() => sortBy(col.key)}>
          <span>{col.label}</span>
          {#if sortKey === col.key}<span class="caret">{sortAsc ? "▲" : "▼"}</span>{/if}
        </button>
      {/each}
      <span class="th c-act"></span>
    </div>

    <div class="body" bind:this={viewport} onscroll={measure}>
      {#if rows.length === 0}
        <div class="empty">
          <h3>{search.running ? "Waiting for peers" : "No matches"}</h3>
        </div>
      {:else}
        <!-- A spacer of the full height gives the scrollbar honest proportions
             while only the visible window is actually rendered. -->
        <div class="spacer" style="height: {rows.length * ROW}px">
          <div class="window" style="transform: translateY({first * ROW}px)">
            {#each visible as row, i (row.username + row.path)}
              <div
                class="row"
                class:alt={(first + i) % 2 === 1}
                ondblclick={() => download(row)}
                role="button"
                tabindex="-1"
                title={row.path}
              >
                <div class="c-name">
                  <span class="fname selectable">{fileName(row.path)}</span>
                  <span class="fpath">{tailPath(parentPath(row.path))}</span>
                </div>
                <div class="c-size num">{bytes(row.size)}</div>
                <div class="c-rate num">
                  {bitrate(row.bitrate)}
                  {#if row.duration}<span class="dim">· {duration(row.duration)}</span>{/if}
                </div>
                <div class="c-user">
                  <span class="uname">{row.username}</span>
                  {#if row.freeSlots > 0}<span class="tag ok">free</span>{/if}
                </div>
                <div class="c-speed num dim">{rate(row.speed)}</div>
                <div class="c-act">
                  <button class="btn small get" onclick={() => download(row)}>Get</button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
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
    padding: 15px 18px 0;
  }

  form {
    display: flex;
    align-items: center;
    gap: 8px;
    position: relative;
  }
  form svg {
    position: absolute;
    left: 10px;
    width: 15px;
    height: 15px;
    fill: var(--text-3);
    pointer-events: none;
  }
  form .field {
    padding-left: 32px;
  }
  form .btn {
    flex: none;
  }

  .tabs {
    display: flex;
    gap: 3px;
    margin-top: 12px;
    padding-bottom: 2px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 190px;
    padding: 5px 11px;
    border-radius: 999px;
    color: var(--text-3);
    font-size: 12.5px;
    transition: color var(--fast), background var(--fast), transform var(--fast);
  }
  .tab:hover {
    background: var(--surface-2);
    color: var(--text-2);
  }
  .tab:active {
    transform: scale(0.97);
  }
  .tab.active {
    background: var(--accent-quiet);
    color: var(--accent);
  }
  .tab .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tab .count {
    font-size: 11px;
    color: var(--text-3);
  }

  .pulse {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 999px;
    background: var(--accent);
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.3;
    }
    50% {
      opacity: 1;
    }
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 18px;
  }
  .field.slim {
    padding: 5px 9px;
    font-size: 12.5px;
    max-width: 240px;
  }
  .field.auto {
    width: auto;
    max-width: none;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-2);
    white-space: nowrap;
    cursor: pointer;
  }
  .check input {
    accent-color: var(--accent);
  }
  .summary {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
  }
  .running {
    color: var(--accent);
  }

  /* One grid definition, shared by the header and every row, so columns
     cannot drift apart. */
  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 82px 140px 170px 92px 56px;
    align-items: center;
    gap: 12px;
    padding: 0 18px;
  }

  .head {
    height: 30px;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    margin: 0 10px;
    padding: 0 8px;
  }
  /*
   * Deliberately not a flex container. Chromium gives `<button>` an internal
   * anonymous wrapper that centres its children, and `justify-content` on the
   * button does not override it — the labels end up centred in their columns.
   * Plain inline flow inside the button sidesteps that entirely, and
   * `text-align` then does the alignment honestly.
   */
  .th {
    display: block;
    padding: 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: left;
    white-space: nowrap;
  }
  .th:hover {
    color: var(--text-2);
  }
  .caret {
    margin-left: 4px;
    font-size: 8px;
  }
  .c-size,
  .c-rate,
  .c-speed {
    justify-content: flex-end;
    text-align: right;
  }
  /* The body cells are flex; the header cells are not, so they need the
     text-align form of the same intent. */
  .head .c-size,
  .head .c-rate,
  .head .c-speed {
    text-align: right;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    position: relative;
  }
  .spacer {
    position: relative;
  }
  .window {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    will-change: transform;
  }

  .row {
    height: 34px;
    cursor: default;
    transition: background var(--fast);
  }
  /* Zebra banding rather than rules: quieter, and easier to track across a
     wide row. It is driven by the row's real index, not its DOM position —
     `:nth-child` would band the sliding window instead of the data, and the
     stripes would swap as you scroll. */
  .row.alt {
    background: color-mix(in srgb, var(--surface-2) 55%, transparent);
  }
  .row:hover {
    background: var(--accent-quiet);
  }
  .row:hover .get {
    opacity: 1;
  }

  .c-name {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .fname {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fpath {
    font-size: 10.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .c-user {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .uname {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .c-size,
  .c-rate,
  .c-speed {
    font-size: 12px;
    color: var(--text-2);
  }
  .dim {
    color: var(--text-3);
  }

  .c-act {
    display: flex;
    justify-content: flex-end;
  }
  .get {
    opacity: 0;
    transition: opacity var(--fast), background var(--fast);
  }
  .get:focus-visible {
    opacity: 1;
  }
</style>
