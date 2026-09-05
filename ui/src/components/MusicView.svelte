<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { player, trackOf } from "../lib/player.svelte";
  import { duration } from "../lib/format";
  import type { Album, AlbumTrack } from "../lib/types";
  import type { Track } from "../lib/player.svelte";

  /**
   * Songs or albums, chosen beside the tabs.
   *
   * One scan feeds both. A song list and a shelf of releases are two ways of
   * looking at the same tags, and reading those tags twice — or keeping a
   * separate folder listing beside them — is how the two drift apart.
   */
  let { shape }: { shape: "songs" | "albums" } = $props();

  let albums = $state<Album[]>([]);
  let loading = $state(true);
  let filter = $state("");
  let sortKey = $state<"artist" | "title" | "album" | "length">("artist");
  /** The release being looked at, or null for the shelf. */
  let open = $state<Album | null>(null);

  /** One track, and the release it belongs to. */
  interface Song {
    track: AlbumTrack;
    album: Album;
  }

  async function reload() {
    loading = true;
    try {
      albums = await core.localAlbums();
    } catch (error) {
      app.notify(String(error), "danger");
      albums = [];
    }
    open = null;
    loading = false;
  }

  onMount(reload);

  function matches(album: Album, track: AlbumTrack | null, needle: string) {
    if (!needle) return true;
    const hay = [album.title, album.artist, track?.title ?? "", track?.name ?? "", track?.artist ?? ""];
    return hay.some((part) => part.toLowerCase().includes(needle));
  }

  const needle = $derived(filter.trim().toLowerCase());

  const shelf = $derived(albums.filter((album) => matches(album, null, needle)));

  const songs = $derived.by(() => {
    const all: Song[] = [];
    for (const album of albums) {
      for (const track of album.tracks) {
        if (matches(album, track, needle)) all.push({ album, track });
      }
    }
    const by = {
      artist: (s: Song) => [
        (s.track.artist ?? s.album.artist).toLowerCase(),
        s.album.title.toLowerCase(),
        s.track.disc ?? 1,
        s.track.number ?? 0,
      ],
      album: (s: Song) => [
        s.album.title.toLowerCase(),
        s.track.disc ?? 1,
        s.track.number ?? 0,
      ],
      title: (s: Song) => [name(s.track).toLowerCase()],
      length: (s: Song) => [-(s.track.seconds ?? 0)],
    }[sortKey];
    return all.sort((a, b) => compare(by(a), by(b)));
  });

  /** Lexicographic over the key each sort produces. */
  function compare(a: (string | number)[], b: (string | number)[]) {
    for (let i = 0; i < a.length; i++) {
      const left = a[i] ?? 0;
      const right = b[i] ?? 0;
      if (left < right) return -1;
      if (left > right) return 1;
    }
    return 0;
  }

  /** A track's own title, or its file name when it has no tags. */
  function name(track: AlbumTrack) {
    return track.title ?? track.name;
  }

  function asTrack(track: AlbumTrack, album: Album): Track {
    return trackOf(
      track.path,
      convertFileSrc(track.path),
      album.downloads ? "downloads" : "shared",
    );
  }

  /** Play one thing, with everything it is listed among behind it. */
  async function play(queue: Track[], at: number) {
    const start = queue[at];
    if (!start) return;
    const played = await player.play(start, queue);
    if (!played) app.notify(`Could not play ${start.name}.`, "danger");
  }

  const playSong = (at: number) =>
    play(
      songs.map((song) => asTrack(song.track, song.album)),
      at,
    );

  const playAlbum = (album: Album, at = 0) =>
    play(
      album.tracks.map((track) => asTrack(track, album)),
      at,
    );

  const total = $derived(
    open?.tracks.reduce((sum, track) => sum + (track.seconds ?? 0), 0) ?? 0,
  );

  // --- windowing, for a song list that can run to tens of thousands ---

  /** Row height in pixels. Has to match `.song` in the stylesheet. */
  const ROW = 40;
  /** Rows kept beyond the viewport, so fast scrolling never shows a gap. */
  const OVERSCAN = 8;

  let scroller = $state<HTMLElement | null>(null);
  let scrolled = $state(0);
  let viewport = $state(0);

  // Measured rather than assumed, and on resize as well as on scroll: with the
  // height only ever read while scrolling, the first paint has nothing to go on
  // and renders one screen of overscan.
  $effect(() => {
    const node = scroller;
    if (!node) return;
    const measure = () => (viewport = node.clientHeight);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  });

  const first = $derived(Math.max(0, Math.floor(scrolled / ROW) - OVERSCAN));
  const last = $derived(
    Math.min(songs.length, Math.ceil((scrolled + viewport) / ROW) + OVERSCAN),
  );
  const window_ = $derived(songs.slice(first, last));
</script>

<!-- One row, wherever a track is listed. The shelf shows the release around
     it, so inside an album the columns that would repeat are left out. -->
{#snippet row(track: AlbumTrack, album: Album, lead: string, showAlbum: boolean, go: () => void)}
  <button class="song" class:current={player.track?.path === track.path} onclick={go}>
    {#if showAlbum}
      <span class="art">
        {#if album.cover}
          <img src={convertFileSrc(album.cover)} alt="" loading="lazy" />
        {:else}
          <span class="blank" aria-hidden="true">♪</span>
        {/if}
        <span class="over" aria-hidden="true">&#9654;</span>
      </span>
    {:else}
      <span class="no num">{lead}</span>
    {/if}
    <span class="title selectable">{name(track)}</span>
    <!-- Inside a release the artist is written above the list already, so it
         only earns a column when a track disagrees with it. -->
    {#if showAlbum}
      <span class="who">{track.artist ?? album.artist}</span>
      <span class="on">{album.title}</span>
    {:else if track.artist && track.artist !== album.artist}
      <span class="who">{track.artist}</span>
    {/if}
    <span class="len num">{track.seconds ? duration(track.seconds) : ""}</span>
  </button>
{/snippet}

<div class="view">
  <header>
    <input
      class="field slim"
      bind:value={filter}
      placeholder={shape === "albums" ? "Filter by album or artist…" : "Filter your music…"}
    />
    {#if shape === "songs"}
      <select class="field slim auto" bind:value={sortKey}>
        <option value="artist">Sort: Artist</option>
        <option value="album">Sort: Album</option>
        <option value="title">Sort: Title</option>
        <option value="length">Sort: Longest</option>
      </select>
    {/if}
    <span class="summary num">
      {#if shape === "albums"}
        {shelf.length.toLocaleString()}
        {shelf.length === 1 ? "release" : "releases"}
      {:else}
        {songs.length.toLocaleString()}
        {songs.length === 1 ? "song" : "songs"}
      {/if}
    </span>
    <button class="btn quiet small" onclick={reload}>Rescan</button>
  </header>

  {#if loading}
    <div class="empty"><h3>Reading tags</h3></div>
  {:else if albums.length === 0}
    <div class="empty">
      <h3>Nothing here yet</h3>
      <p>Music from your downloads and the folders you share appears here.</p>
    </div>
  {:else if shape === "albums" && open}
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
          <div class="blank big" aria-hidden="true">♪</div>
        {/if}
        <div class="facts">
          <h2 class="selectable">{open.title}</h2>
          <p class="by selectable">{open.artist}</p>
          <p class="num spec">
            {open.year ? `${open.year} · ` : ""}{open.tracks.length}
            {open.tracks.length === 1 ? "track" : "tracks"}{total ? ` · ${duration(total)}` : ""}
          </p>
          <button class="btn primary small" onclick={() => playAlbum(open!)}>Play</button>
        </div>
      </div>

      <div class="rows">
        {#each open.tracks as track, index (track.path)}
          {@render row(
            track,
            open,
            String(track.number ?? index + 1),
            false,
            () => playAlbum(open!, index),
          )}
        {/each}
      </div>
    </div>
  {:else if shape === "albums"}
    {#if shelf.length === 0}
      <div class="empty"><h3>No matches</h3></div>
    {:else}
      <div class="grid">
        {#each shelf as album (album.key)}
          <button class="card" onclick={() => (open = album)}>
            <span class="art big">
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
  {:else if songs.length === 0}
    <div class="empty"><h3>No matches</h3></div>
  {:else}
    <!-- Windowed: a library of thirty thousand songs is one `<div>` tall and
         only the rows on screen exist. -->
    <div
      class="rows scroll"
      bind:this={scroller}
      onscroll={(event) => (scrolled = event.currentTarget.scrollTop)}
    >
      <div class="spacer" style="height: {songs.length * ROW}px">
        <div class="slice" style="transform: translateY({first * ROW}px)">
          {#each window_ as song, index (song.track.path)}
            {@render row(song.track, song.album, "", true, () => playSong(first + index))}
          {/each}
        </div>
      </div>
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
    gap: 10px;
    padding: 12px 18px;
  }
  header .field {
    width: 25%;
    min-width: 220px;
    max-width: 380px;
  }
  header .field.auto {
    width: auto;
    min-width: 0;
  }
  .summary {
    flex: 1;
    font-size: 12px;
    color: var(--text-3);
  }

  /* --- one row, both places --- */
  .rows {
    border-radius: var(--radius);
    background: var(--surface-2);
    overflow: hidden;
  }
  .rows.scroll {
    flex: 1;
    min-height: 0;
    margin: 0 18px 18px;
    overflow-y: auto;
  }
  .spacer {
    position: relative;
  }
  .slice {
    position: absolute;
    top: 0;
    width: 100%;
  }
  .song {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 40px;
    padding: 0 14px;
    text-align: left;
    font-size: 12.5px;
  }
  .song:hover {
    background: var(--accent-quiet);
  }
  .song.current .title {
    color: var(--accent);
    font-weight: 500;
  }
  .no {
    flex: none;
    width: 22px;
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
  .who,
  .on {
    flex: none;
    width: 24%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .len {
    flex: none;
    width: 44px;
    color: var(--text-3);
    text-align: right;
  }

  /* --- artwork, at three sizes --- */
  .art {
    position: relative;
    flex: none;
    display: block;
    width: 28px;
    height: 28px;
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-3);
  }
  /* The sleeve is the row's identity, so pressing it has to look like it does
     something. It says so only under the pointer. */
  .over {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #000000a8;
    color: #fff;
    font-size: 9px;
    opacity: 0;
    transition: opacity var(--fast);
  }
  .song:hover .over {
    opacity: 1;
  }
  .art.big {
    width: 100%;
    height: auto;
    aspect-ratio: 1;
    margin-bottom: 5px;
    border-radius: var(--radius);
    background: var(--surface-2);
    box-shadow: var(--shadow);
    transition: transform var(--fast);
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
    font-size: 13px;
  }
  .blank.big {
    font-size: 40px;
  }

  /* --- the shelf --- */
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 18px 16px;
    padding: 0 18px 20px;
    align-content: start;
  }
  .card {
    display: grid;
    gap: 3px;
    text-align: left;
  }
  .card:hover .art.big {
    transform: translateY(-2px);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-weight: 500;
  }
  .card .who {
    width: auto;
  }

  /* --- one release --- */
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
    padding: 0 6px 10px 0;
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
  .sleeve .blank.big {
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
</style>
