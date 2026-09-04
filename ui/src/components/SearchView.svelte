<script lang="ts">
  import { core } from "../lib/core";
  import { app, AppState, type ResultRow } from "../lib/state.svelte";
  import { bitrate, bytes, duration, fileName, format, parentPath, rate, tailPath } from "../lib/format";

  let query = $state("");
  let filter = $state("");
  let readyOnly = $state(false);
  let chosen = $state("any");
  let sortKey = $state<"name" | "size" | "bitrate" | "speed" | "user">("speed");
  let sortAsc = $state(false);
  /** Collapsed nodes, by key. Absent means open. */
  let closed = $state<Record<string, true>>({});

  /**
   * Row heights. A peer heads a section and a folder groups one, so both are
   * given the weight to be found while scrolling past; a file is a file.
   */
  const HEIGHT = { user: 68, folder: 51, file: 34 } as const;
  /** Rows rendered beyond the viewport, so fast scrolling never shows gaps. */
  const OVERSCAN = 6;

  let viewport = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const search = $derived(app.search);

  const formats = $derived.by(() => {
    const seen = new Set<string>();
    for (const row of search?.rows ?? []) {
      const kind = format(row.path);
      if (kind) seen.add(kind);
    }
    return [...seen].sort();
  });

  const rows = $derived.by(() => {
    let list = search?.rows ?? [];

    const needle = filter.trim().toLowerCase();
    if (needle) list = list.filter((r) => r.path.toLowerCase().includes(needle));
    if (readyOnly) list = list.filter((r) => r.freeSlots > 0);
    if (chosen !== "any") list = list.filter((r) => format(r.path) === chosen);

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

  /**
   * One line of the grouped view.
   *
   * The tree is flattened to a list rather than nested markup so the same
   * fixed-height windowing works on it: a peer sharing a thousand files is
   * one array either way, and only the lines on screen are ever built.
   */
  type Line =
    | { kind: "user"; key: string; username: string; files: number; freeSlots: number; speed: number }
    | { kind: "folder"; key: string; username: string; folder: string; files: number; size: number }
    | { kind: "file"; key: string; row: ResultRow };

  /** Group preserving first appearance, so the chosen sort still shows. */
  function groupBy<T>(items: T[], by: (item: T) => string): Map<string, T[]> {
    const out = new Map<string, T[]>();
    for (const item of items) {
      const key = by(item);
      const existing = out.get(key);
      if (existing) existing.push(item);
      else out.set(key, [item]);
    }
    return out;
  }

  const lines = $derived.by(() => {
    const out: Line[] = [];
    for (const [username, mine] of groupBy(rows, (r) => r.username)) {
      const userKey = `u ${username}`;
      // Slots and speed describe the peer, so any of its rows will do.
      // `groupBy` never makes an empty group, but the compiler cannot know it.
      const peer = mine[0];
      if (!peer) continue;
      out.push({
        kind: "user",
        key: userKey,
        username,
        files: mine.length,
        freeSlots: peer.freeSlots,
        speed: peer.speed,
      });
      if (closed[userKey]) continue;

      for (const [folder, here] of groupBy(mine, (r) => parentPath(r.path))) {
        const folderKey = `f ${username} ${folder}`;
        out.push({
          kind: "folder",
          key: folderKey,
          username,
          folder,
          files: here.length,
          size: here.reduce((total, r) => total + r.size, 0),
        });
        if (closed[folderKey]) continue;
        for (const row of here) {
          out.push({ kind: "file", key: `x ${username} ${row.path}`, row });
        }
      }
    }
    return out;
  });

  /**
   * Where each line starts, and how tall the list is.
   *
   * Rows are no longer one height, so the window cannot be found by dividing.
   * The offsets are built in the pass that builds the lines and searched when
   * scrolling, so the scroll handler still does no work proportional to the
   * length of the list.
   */
  const layout = $derived.by(() => {
    const tops = new Array<number>(lines.length);
    let y = 0;
    for (const [index, line] of lines.entries()) {
      tops[index] = y;
      y += HEIGHT[line.kind];
    }
    return { tops, total: y };
  });

  /** The last line starting at or before `y`. */
  function lineAt(tops: number[], y: number): number {
    let low = 0;
    let high = tops.length - 1;
    while (low < high) {
      const mid = Math.ceil((low + high) / 2);
      if ((tops[mid] ?? 0) <= y) low = mid;
      else high = mid - 1;
    }
    return Math.max(0, low);
  }

  const first = $derived(Math.max(0, lineAt(layout.tops, scrollTop) - OVERSCAN));
  const last = $derived(
    Math.min(lines.length, lineAt(layout.tops, scrollTop + viewportHeight) + OVERSCAN + 1),
  );
  const visibleLines = $derived(lines.slice(first, last));
  const offset = $derived(layout.tops[first] ?? 0);

  function toggle(key: string) {
    // Reassigned rather than mutated: the record is what the tree reads.
    if (closed[key]) {
      const { [key]: _removed, ...rest } = closed;
      closed = rest;
    } else {
      closed = { ...closed, [key]: true };
    }
  }

  function measure() {
    if (!viewport) return;
    scrollTop = viewport.scrollTop;
    viewportHeight = viewport.clientHeight;
  }

  // `measure` used to run only on scroll, so until the first scroll the height
  // was still zero and the window was nothing but overscan — twelve rows, on a
  // viewport with room for forty. It also never noticed a resized window.
  // Measuring when the element arrives and watching it thereafter fixes both.
  $effect(() => {
    const element = viewport;
    if (!element) return;
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  });

  function sortBy(key: typeof sortKey) {
    sortKey = key;
    // Names read best A–Z; every numeric field reads best largest-first.
    sortAsc = key === "name" || key === "user";
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const text = query.trim();
    if (!text) return;
    try {
      const id = await core.search(text);
      app.startSearch(id, text);
      filter = "";
      chosen = "any";
      if (viewport) viewport.scrollTop = 0;
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  /** Stop a running search. Hits already delivered stay on screen. */
  async function stop(id: number) {
    try {
      await core.cancelSearch(id);
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

  /**
   * Queue every file of one peer's folder.
   *
   * Sent in one go and left to the core's slot limit to pace, which is what
   * that limit is for — the alternative is asking the interface to invent a
   * second, quieter queue that disagrees with the real one.
   */
  async function downloadFolder(username: string, folder: string) {
    const here = rows.filter((r) => r.username === username && parentPath(r.path) === folder);
    try {
      for (const row of here) await core.download(row.username, row.path, row.size);
      app.notify(`Queued ${here.length} file${here.length === 1 ? "" : "s"}`);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  /** Sort choices, now that there is no column header to click. */
  const SORTS: { key: typeof sortKey; label: string }[] = [
    { key: "speed", label: "Speed" },
    { key: "size", label: "Size" },
    { key: "bitrate", label: "Bitrate" },
    { key: "name", label: "Name" },
    { key: "user", label: "User" },
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
      <select class="field slim auto" bind:value={chosen}>
        <option value="any">Any format</option>
        {#each formats as ext (ext)}
          <option value={ext}>{ext.toUpperCase()}</option>
        {/each}
      </select>
      <label class="check">
        <input type="checkbox" bind:checked={readyOnly} />
        <span>Free slots only</span>
      </label>
      <select
        class="field slim auto"
        value={sortKey}
        onchange={(event) => sortBy(event.currentTarget.value as typeof sortKey)}
      >
        {#each SORTS as option (option.key)}
          <option value={option.key}>Sort: {option.label}</option>
        {/each}
      </select>
      <button
        class="btn quiet small"
        onclick={() => (sortAsc = !sortAsc)}
        title={sortAsc ? "Ascending" : "Descending"}
        aria-label={sortAsc ? "Ascending" : "Descending"}
      >
        {sortAsc ? "▲" : "▼"}
      </button>
      <span class="summary num">
        {rows.length.toLocaleString()} of {search.rows.length.toLocaleString()}
        {#if search.running}<span class="running">· searching…</span>{/if}
      </span>
      {#if search.running}
        <button class="btn quiet small" onclick={() => stop(search.id)}>Stop</button>
      {/if}
    </div>


    <div class="body" bind:this={viewport} onscroll={measure}>
      {#if rows.length === 0}
        <div class="empty">
          <h3>{search.running ? "Waiting for peers" : "No matches"}</h3>
        </div>
      {:else}
        <!-- Windowed as before, but the rows are no longer one height, so the
             spacer and the offset come from measured tops. -->
        <div class="spacer" style="height: {layout.total}px">
          <div class="window" style="transform: translateY({offset}px)">
            {#each visibleLines as line (line.key)}
              {#if line.kind === "user"}
                <button class="tline tuser" onclick={() => toggle(line.key)}>
                  <span class="chev" class:open={!closed[line.key]} aria-hidden="true">▶</span>
                  <span class="uname">{line.username}</span>
                  <!-- Free slots, not presence: every peer answering a search
                       is online, and this says whether one can start now. -->
                  {#if line.freeSlots > 0}<span class="tag ok">free</span>{/if}
                  <span class="tmeta num">{line.files.toLocaleString()} files</span>
                  <span class="tmeta num dim">{rate(line.speed)}</span>
                </button>
              {:else if line.kind === "folder"}
                <div class="tline tfolder">
                  <button class="tgrip" onclick={() => toggle(line.key)}>
                    <span class="chev" class:open={!closed[line.key]} aria-hidden="true">▶</span>
                    <span class="fold selectable" title={line.folder}>
                      {tailPath(line.folder, 3) || "(root)"}
                    </span>
                    <span class="tmeta num">{line.files.toLocaleString()}</span>
                    <span class="tmeta num dim">{bytes(line.size)}</span>
                  </button>
                  <button
                    class="btn small get"
                    onclick={() => downloadFolder(line.username, line.folder)}
                  >
                    Get folder
                  </button>
                </div>
              {:else}
                <div
                  class="tline tfile"
                  ondblclick={() => download(line.row)}
                  role="button"
                  tabindex="-1"
                  title={line.row.path}
                >
                  {#if app.downloaded.has(AppState.had(fileName(line.row.path), line.row.size))}
                    <span class="had" title="Already in your download folder">✓</span>
                  {/if}
                  <span class="fname selectable">{fileName(line.row.path)}</span>
                  <span class="tmeta num">{bytes(line.row.size)}</span>
                  <span class="tmeta kind">{format(line.row.path).toUpperCase()}</span>
                  <span class="tmeta num">
                    {bitrate(line.row.bitrate)}
                    {#if line.row.duration}<span class="dim">· {duration(line.row.duration)}</span>{/if}
                  </span>
                  <button class="btn small get" onclick={() => download(line.row)}>Get</button>
                </div>
              {/if}
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* These heights are the ones `HEIGHT` names in the script. They have to
     agree: the window is positioned by arithmetic on those numbers, so a row
     that renders taller than it was measured drifts away from its slot. */
  .tline {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 14px;
    text-align: left;
  }
  .tuser {
    gap: 8px;
    height: 68px;
    padding: 0 14px;
    background: var(--surface-2);
    font-weight: 500;
  }
  .tuser .uname {
    font-size: 14px;
  }
  .tfolder {
    height: 51px;
  }
  .tfolder .fold {
    font-size: 13px;
  }
  .tuser:hover {
    background: var(--surface-3);
  }
  .tuser .uname {
    font-size: 12.5px;
  }

  .tfolder {
    padding-right: 10px;
    padding-left: 26px;
  }
  .tfolder:hover,
  .tfile:hover {
    background: var(--accent-quiet);
  }
  /* The grip is the whole clickable span of the folder line, so the button
     beside it stays a separate target rather than swallowing the toggle. */
  .tgrip {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    height: 100%;
    text-align: left;
  }
  .fold {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    color: var(--text-2);
  }

  .tfile {
    padding-left: 46px;
  }
  .tfile .fname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }

  /* Actions appear on hover, as they do on a flat row — and now on keyboard
     focus too, which the flat rows never handled: a button at `opacity: 0` is
     still focusable and still clickable, so tabbing to one was reaching
     something invisible. */
  .tline .get {
    opacity: 0;
    transition: opacity var(--fast);
  }
  .tline:hover .get,
  .tline .get:focus-visible {
    opacity: 1;
  }

  /* The format sits with the size rather than in the name: a peer's naming is
     unreliable, and the extension is the one part that is not a description. */
  /* A file already on disk. Ahead of the name rather than after it, so a
     folder of them reads as a column at a glance instead of needing the eye
     to travel each line. */
  .had {
    flex: none;
    width: 12px;
    margin-right: -4px;
    font-size: 11px;
    color: var(--ok);
  }

  .kind {
    width: 46px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.03em;
    color: var(--text-3);
    text-align: right;
  }

  .tmeta {
    flex: none;
    font-size: 11.5px;
    color: var(--text-2);
    text-align: right;
  }
  .chev {
    flex: none;
    width: 8px;
    font-size: 8px;
    color: var(--text-3);
    transition: transform var(--fast);
  }
  .chev.open {
    transform: rotate(90deg);
  }

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
  .fname {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .uname {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dim {
    color: var(--text-3);
  }
  .get {
    opacity: 0;
    transition: opacity var(--fast), background var(--fast);
  }
  .get:focus-visible {
    opacity: 1;
  }
</style>
