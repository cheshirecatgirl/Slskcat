<script lang="ts">
  import { core } from "../lib/core";
  import { app, type ResultRow } from "../lib/state.svelte";
  import { bitrate, bytes, duration, fileName, parentPath, rate, tailPath } from "../lib/format";

  let draft = $state("");
  let open = $state<string | null>(null);

  const wishes = $derived(app.settings?.wishlist ?? []);

  /** Every wish is re-sent by the server on its own schedule. */
  const cadence = $derived.by(() => {
    const seconds = app.wishInterval;
    if (seconds === null) return null;
    const minutes = Math.round(seconds / 60);
    return minutes >= 60
      ? `every ${Math.round(minutes / 60)} h`
      : `every ${minutes} min`;
  });

  /** Persist the set and hand it to the core, which restates it whole. */
  async function commit(queries: string[]) {
    if (!app.settings) return;
    try {
      app.settings = await core.saveSettings({ ...app.settings, wishlist: queries });
      await core.setWishlist(queries);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  async function add(event: SubmitEvent) {
    event.preventDefault();
    const query = draft.trim();
    if (!query || wishes.includes(query)) {
      draft = "";
      return;
    }
    await commit([...wishes, query]);
    draft = "";
    open = query;
  }

  async function remove(query: string) {
    await commit(wishes.filter((w) => w !== query));
    if (open === query) open = null;
  }

  async function get(row: ResultRow) {
    try {
      await core.download(row.username, row.path, row.size);
      app.notify(`Queued ${fileName(row.path)}`);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }
</script>

<div class="view">
  <header>
    <form onsubmit={add}>
      <input
        class="field"
        bind:value={draft}
        placeholder="Something the network doesn't have yet…"
        spellcheck="false"
        autocapitalize="off"
      />
      <button class="btn primary" type="submit" disabled={!draft.trim()}>Add wish</button>
    </form>
    <p class="note">
      The server re-runs these on its own schedule{cadence ? `, ${cadence}` : ""}. Hits collect
      here while you get on with something else.
    </p>
  </header>

  {#if wishes.length === 0}
    <div class="empty"><h3>No wishes</h3></div>
  {:else}
    <div class="body">
      {#each wishes as wish (wish)}
        {@const hits = app.wishHits[wish] ?? []}
        <div class="wish">
          <button class="line" onclick={() => (open = open === wish ? null : wish)}>
            <span class="chev" class:open={open === wish} aria-hidden="true">▶</span>
            <span class="query selectable">{wish}</span>
            {#if hits.length > 0}
              <span class="found num">{hits.length.toLocaleString()} found</span>
            {:else}
              <span class="waiting">waiting</span>
            {/if}
            <span
              class="btn quiet small"
              role="button"
              tabindex="0"
              onclick={(e) => {
                e.stopPropagation();
                void remove(wish);
              }}
              onkeydown={(e) => e.key === "Enter" && remove(wish)}
            >
              Remove
            </span>
          </button>

          {#if open === wish && hits.length > 0}
            <div class="hits">
              {#each hits.slice(0, 200) as row (row.username + row.path)}
                <div class="hit">
                  <div class="what">
                    <span class="name selectable">{fileName(row.path)}</span>
                    <span class="where">{tailPath(parentPath(row.path))}</span>
                  </div>
                  <span class="num meta">{bytes(row.size)}</span>
                  <span class="num meta">
                    {bitrate(row.bitrate)}
                    {#if row.duration}<span class="dim">· {duration(row.duration)}</span>{/if}
                  </span>
                  <span class="who">
                    {row.username}
                    {#if row.freeSlots > 0}<span class="tag ok">free</span>{/if}
                  </span>
                  <span class="num meta dim">{rate(row.speed)}</span>
                  <button class="btn small" onclick={() => get(row)}>Get</button>
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
    padding: 12px 18px 12px;
  }
  form {
    display: flex;
    gap: 8px;
  }
  form .btn {
    flex: none;
  }
  .note {
    margin-top: 9px;
    max-width: 62ch;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-3);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 12px;
  }

  .wish {
    margin: 0 10px 4px;
  }

  .line {
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
    text-align: left;
    transition: background var(--fast);
  }
  .line:hover {
    background: var(--surface-3);
  }
  .chev {
    font-size: 8px;
    color: var(--text-3);
    transition: transform var(--fast);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .query {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .found {
    font-size: 11.5px;
    color: var(--accent);
    font-weight: 500;
  }
  .waiting {
    font-size: 11.5px;
    color: var(--text-3);
  }

  .hits {
    margin: 3px 0 8px;
    border-radius: var(--radius);
    background: var(--bg);
    overflow: hidden;
  }
  .hit {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 82px 132px 160px 88px 52px;
    align-items: center;
    gap: 12px;
    padding: 6px 12px;
  }
  .hit:hover {
    background: var(--accent-quiet);
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .name {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .where {
    font-size: 10.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 12px;
    color: var(--text-2);
    text-align: right;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: 12.5px;
  }
  .dim {
    color: var(--text-3);
  }
</style>
