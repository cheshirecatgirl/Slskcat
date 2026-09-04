<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { player, trackOf } from "../lib/player.svelte";
  import { bytes, format } from "../lib/format";
  import type { LocalFile, LocalRoot } from "../lib/types";

  let roots = $state<LocalRoot[]>([]);
  let loading = $state(true);
  let filter = $state("");
  let closed = $state<Record<string, true>>({});

  /**
   * Formats worth handing to a player.
   *
   * A library lists everything in the folder, cue sheets and logs and artwork
   * included, because it is a view of what is there. Only these get a play
   * control.
   */
  const PLAYABLE = new Set([
    "flac",
    "mp3",
    "m4a",
    "aac",
    "ogg",
    "oga",
    "opus",
    "wav",
    "aiff",
    "aif",
  ]);

  async function reload() {
    try {
      roots = await core.localLibrary();
    } catch (error) {
      app.notify(String(error), "danger");
      roots = [];
    }
    loading = false;
  }

  onMount(reload);

  interface Group {
    key: string;
    root: LocalRoot;
    folder: string;
    files: LocalFile[];
  }

  const groups = $derived.by(() => {
    const needle = filter.trim().toLowerCase();
    const out: Group[] = [];
    for (const root of roots) {
      const matched = needle
        ? root.files.filter(
            (file) =>
              file.name.toLowerCase().includes(needle) || file.folder.toLowerCase().includes(needle),
          )
        : root.files;
      const byFolder = new Map<string, LocalFile[]>();
      for (const file of matched) {
        const existing = byFolder.get(file.folder);
        if (existing) existing.push(file);
        else byFolder.set(file.folder, [file]);
      }
      for (const [folder, files] of byFolder) {
        out.push({ key: `${root.path} ${folder}`, root, folder, files });
      }
    }
    return out;
  });

  const counted = $derived(roots.reduce((total, root) => total + root.files.length, 0));

  function toggle(key: string) {
    if (closed[key]) {
      const { [key]: _gone, ...rest } = closed;
      closed = rest;
    } else {
      closed = { ...closed, [key]: true };
    }
  }

  function play(file: LocalFile, root: LocalRoot) {
    // `convertFileSrc` turns a path into something an `<audio>` element will
    // load; the webview cannot open a file by name. It resolves only inside
    // the scope the app grants at startup, which is exactly these folders.
    void player.play(
      trackOf(file.path, convertFileSrc(file.path), root.downloads ? "downloads" : "shared"),
    );
  }
</script>

<div class="view">
  <header>
    <input class="field" bind:value={filter} placeholder="Filter your files…" />
    <span class="summary num">
      {counted.toLocaleString()} files in {roots.length.toLocaleString()}
      {roots.length === 1 ? "folder" : "folders"}
    </span>
    <button class="btn quiet small" onclick={reload}>Rescan</button>
  </header>

  {#if loading}
    <div class="empty"><h3>Reading your folders</h3></div>
  {:else if groups.length === 0}
    <div class="empty">
      <h3>{counted === 0 ? "Nothing here yet" : "No matches"}</h3>
      {#if counted === 0}
        <p>Finished downloads and the folders you share both appear here.</p>
      {/if}
    </div>
  {:else}
    <div class="body">
      {#each groups as group (group.key)}
        <div class="folder">
          <button class="dirline" onclick={() => toggle(group.key)}>
            <span class="chev" class:open={!closed[group.key]} aria-hidden="true">▶</span>
            <span class="tag" class:ok={group.root.downloads}>
              {group.root.downloads ? "downloads" : "shared"}
            </span>
            <span class="dirname selectable" title={group.root.path}>
              {group.folder || "(top level)"}
            </span>
            <span class="dircount num">{group.files.length}</span>
          </button>

          {#if !closed[group.key]}
            <div class="files">
              {#each group.files as file (file.path)}
                <div class="file" class:current={player.track?.path === file.path}>
                  {#if PLAYABLE.has(format(file.name))}
                    <button
                      class="go"
                      onclick={() => play(file, group.root)}
                      title="Play {file.name}"
                      aria-label="Play {file.name}">&#9654;</button
                    >
                  {:else}
                    <span class="go blank" aria-hidden="true"></span>
                  {/if}
                  <span class="fname selectable">{file.name}</span>
                  <span class="meta kind">{format(file.name).toUpperCase()}</span>
                  <span class="meta num">{bytes(file.size)}</span>
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
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 15px 18px 12px;
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

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 12px;
  }
  .folder {
    margin: 0 10px 4px;
  }
  .dirline {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
    text-align: left;
    transition: background var(--fast);
  }
  .dirline:hover {
    background: var(--surface-3);
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
  .dirname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .dircount {
    flex: none;
    font-size: 11.5px;
    color: var(--text-3);
  }

  .files {
    margin: 3px 0 8px;
    border-radius: var(--radius);
    background: var(--bg);
    overflow: hidden;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 12px;
  }
  .file:hover,
  .file.current {
    background: var(--accent-quiet);
  }
  .file.current .fname {
    color: var(--accent);
    font-weight: 500;
  }
  .go {
    flex: none;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    font-size: 9px;
    color: var(--text-3);
    transition: background var(--fast), color var(--fast);
  }
  .go.blank {
    pointer-events: none;
  }
  .file:hover .go:not(.blank) {
    background: var(--accent);
    color: #fff;
  }
  .fname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .meta {
    flex: none;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .kind {
    width: 46px;
    letter-spacing: 0.03em;
    text-align: right;
  }
  .meta.num {
    width: 72px;
    text-align: right;
  }
</style>
