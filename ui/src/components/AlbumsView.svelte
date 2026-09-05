<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { player, trackOf } from "../lib/player.svelte";
  import { duration } from "../lib/format";
  import type { Album, AlbumTrack } from "../lib/types";

  let albums = $state<Album[]>([]);
  let loading = $state(true);
  let filter = $state("");
  /** The release being looked at, or null for the grid. */
  let open = $state<Album | null>(null);

  async function reload() {
    loading = true;
    try {
      albums = await core.localAlbums();
    } catch (error) {
      app.notify(String(error), "danger");
      albums = [];
    }
    loading = false;
  }

  onMount(reload);

  const shown = $derived.by(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) return albums;
    return albums.filter(
      (album) =>
        album.title.toLowerCase().includes(needle) ||
        album.artist.toLowerCase().includes(needle),
    );
  });

  /** A track's own title, or its file name when it has no tags. */
  function label(track: AlbumTrack) {
    return track.title ?? track.name;
  }

  function tracksOf(album: Album) {
    return album.tracks.map((track) =>
      trackOf(track.path, convertFileSrc(track.path), album.downloads ? "downloads" : "shared"),
    );
  }

  async function play(album: Album, from = 0) {
    const queue = tracksOf(album);
    const start = queue[from];
    if (!start) return;
    // The whole release goes with it, so looping the folder loops the album.
    const played = await player.play(start, queue);
    if (!played) app.notify(`Could not play ${start.name}.`, "danger");
  }

  const total = $derived(
    open?.tracks.reduce((sum, track) => sum + (track.seconds ?? 0), 0) ?? 0,
  );
</script>

<div class="view">
  <header>
    <input class="field" bind:value={filter} placeholder="Filter by album or artist…" />
    <span class="summary num">
      {albums.length.toLocaleString()}
      {albums.length === 1 ? "release" : "releases"}
    </span>
    <button class="btn quiet small" onclick={reload}>Rescan</button>
  </header>

  {#if loading}
    <div class="empty"><h3>Reading tags</h3></div>
  {:else if open}
    <div class="detail">
      <button class="back" onclick={() => (open = null)}>
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M11 4.6 4.3 11.3a1 1 0 0 0 0 1.4L11 19.4l1.4-1.4-4.3-4.3H20v-2H8.1l4.3-4.3L11 4.6Z" />
        </svg>
        All releases
      </button>

      <div class="sleeve">
        {#if open.cover}
          <img src={convertFileSrc(open.cover)} alt="" />
        {:else}
          <div class="blank" aria-hidden="true">♪</div>
        {/if}
        <div class="facts">
          <h2 class="selectable">{open.title}</h2>
          <p class="by selectable">{open.artist}</p>
          <p class="num spec">
            {open.year ? `${open.year} · ` : ""}{open.tracks.length}
            {open.tracks.length === 1 ? "track" : "tracks"}{total
              ? ` · ${duration(total)}`
              : ""}
          </p>
          <button class="btn primary small" onclick={() => play(open!)}>Play</button>
        </div>
      </div>

      <div class="tracks">
        {#each open.tracks as track, index (track.path)}
          <button
            class="track"
            class:current={player.track?.path === track.path}
            onclick={() => play(open!, index)}
          >
            <span class="no num">{track.number ?? index + 1}</span>
            <span class="title selectable">{label(track)}</span>
            {#if track.artist && track.artist !== open.artist}
              <span class="who">{track.artist}</span>
            {/if}
            <span class="len num">{track.seconds ? duration(track.seconds) : ""}</span>
          </button>
        {/each}
      </div>
    </div>
  {:else if shown.length === 0}
    <div class="empty">
      <h3>{albums.length === 0 ? "Nothing here yet" : "No matches"}</h3>
      {#if albums.length === 0}
        <p>Music from your downloads and the folders you share appears here.</p>
      {/if}
    </div>
  {:else}
    <div class="grid">
      {#each shown as album (album.key)}
        <button class="card" onclick={() => (open = album)}>
          <span class="art">
            {#if album.cover}
              <img src={convertFileSrc(album.cover)} alt="" loading="lazy" />
            {:else}
              <span class="blank" aria-hidden="true">♪</span>
            {/if}
          </span>
          <span class="name">{album.title}</span>
          <span class="who">{album.artist}</span>
        </button>
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
    padding: 12px 18px 12px;
  }
  header .field {
    width: 25%;
    min-width: 240px;
    max-width: 420px;
  }
  .summary {
    flex: 1;
    font-size: 12px;
    color: var(--text-3);
  }

  /* Covers are square, so the grid is: as many as fit, each as wide as it is
     tall, and the row heights follow from that rather than being set. */
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 18px 16px;
    padding: 4px 18px 20px;
    align-content: start;
  }
  .card {
    display: grid;
    gap: 3px;
    text-align: left;
  }
  .art {
    display: block;
    aspect-ratio: 1;
    margin-bottom: 5px;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: var(--shadow);
    transition: transform var(--fast);
  }
  .card:hover .art {
    transform: translateY(-2px);
  }
  .art img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .blank {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    color: var(--text-3);
    font-size: 28px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-weight: 500;
  }
  .who {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11.5px;
    color: var(--text-3);
  }

  .detail {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 18px 20px;
  }
  .back {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 6px 8px 0;
    font-size: 12.5px;
    color: var(--text-3);
    transition: color var(--fast);
  }
  .back svg {
    width: 14px;
    height: 14px;
    fill: currentColor;
    transition: transform var(--fast);
  }
  .back:hover {
    color: var(--text-1);
  }
  .back:hover svg {
    transform: translateX(-2px);
  }

  .sleeve {
    display: flex;
    align-items: flex-end;
    gap: 18px;
    margin-bottom: 18px;
  }
  .sleeve img,
  .sleeve .blank {
    flex: none;
    width: 168px;
    height: 168px;
    border-radius: var(--radius);
    object-fit: cover;
    background: var(--surface-2);
    box-shadow: var(--shadow);
  }
  .facts {
    display: grid;
    gap: 5px;
    min-width: 0;
  }
  .facts h2 {
    font-size: 21px;
    font-weight: 600;
    letter-spacing: -0.015em;
  }
  .facts .by {
    font-size: 13px;
    color: var(--text-2);
  }
  /* Not `.small`: the Play button below carries that as a size modifier, and
     a colour set here would win over the one `.btn.primary` sets. */
  .facts .spec {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .facts .btn {
    justify-self: start;
    margin-top: 5px;
  }

  .tracks {
    border-radius: var(--radius);
    background: var(--surface-2);
    overflow: hidden;
  }
  .track {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 34px;
    padding: 0 14px;
    text-align: left;
    font-size: 12.5px;
  }
  .track:hover {
    background: var(--accent-quiet);
  }
  .track.current .title {
    color: var(--accent);
    font-weight: 500;
  }
  .no {
    flex: none;
    width: 20px;
    color: var(--text-3);
    text-align: right;
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track .who {
    flex: none;
    max-width: 30%;
  }
  .len {
    flex: none;
    color: var(--text-3);
  }
</style>
