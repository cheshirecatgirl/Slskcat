<script lang="ts">
  import { onMount } from "svelte";
  import { core, onEvent } from "./lib/core";
  import { app } from "./lib/state.svelte";
  import type { Section } from "./lib/nav";
  import Sidebar from "./components/Sidebar.svelte";
  import Notices from "./components/Notices.svelte";
  import CommandBar from "./components/CommandBar.svelte";
  import ConnectView from "./components/ConnectView.svelte";
  import SearchView from "./components/SearchView.svelte";
  import WishlistView from "./components/WishlistView.svelte";
  import TransfersView from "./components/TransfersView.svelte";
  import BrowseView from "./components/BrowseView.svelte";
  import MessagesView from "./components/MessagesView.svelte";
  import SettingsView from "./components/SettingsView.svelte";

  let section = $state<Section>("search");
  let palette = $state(false);

  onMount(() => {
    // Preferences first: the sign-in form and the settings screen both render
    // from them, and a failed load must be visible rather than silently
    // resetting what the user configured.
    core
      .loadSettings()
      .then((loaded) => (app.settings = loaded))
      .catch((error) => {
        app.settingsError = String(error);
        app.notify(`Could not load settings: ${error}`, "danger");
      });

    // The subscription resolves asynchronously; unsubscribing has to wait for
    // it, so the cleanup awaits the same promise rather than racing it.
    const subscription = onEvent((event) => app.apply(event));
    return () => {
      void subscription.then((unlisten) => unlisten());
    };
  });

  function onKey(event: KeyboardEvent) {
    // Cmd/Ctrl-K opens the palette from anywhere, the way Arc's command bar
    // does. Escape is handled inside the palette itself.
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (app.connected) palette = !palette;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- The section drives the accent and the colour of the field behind it. -->
<div class="window" data-space={app.connected ? section : "search"}>
  <div class="field-glow" aria-hidden="true"></div>

  {#if app.connected}
    <Sidebar bind:section onCommand={() => (palette = true)} />
  {/if}

  <div class="canvas" class:solo={!app.connected}>
    {#if !app.connected}
      <ConnectView />
    {:else if section === "search"}
      <SearchView onCommand={() => (palette = true)} />
    {:else if section === "wishlist"}
      <WishlistView />
    {:else if section === "transfers"}
      <TransfersView />
    {:else if section === "browse"}
      <BrowseView />
    {:else if section === "messages"}
      <MessagesView />
    {:else}
      <SettingsView />
    {/if}
  </div>

  {#if palette}
    <CommandBar
      bind:section
      onclose={() => (palette = false)}
    />
  {/if}

  <Notices />
</div>

<style>
  .window {
    position: relative;
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    height: 100%;
    padding: var(--gap) var(--gap) var(--gap) 0;
    background: var(--bg);
    /* The accent transition is what sells moving between sections. */
    transition: --accent var(--slow);
  }
  .window:has(.canvas.solo) {
    grid-template-columns: minmax(0, 1fr);
    padding: var(--gap);
  }

  /*
   * The coloured field. Two soft radial washes tinted by the active section,
   * sitting behind everything. Kept as its own layer so only this element
   * repaints when the section changes.
   */
  .field-glow {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      radial-gradient(115% 85% at 3% 0%, var(--space-a), transparent 62%),
      radial-gradient(95% 75% at 100% 100%, var(--space-b), transparent 58%);
    transition: background var(--slow);
  }

  /*
   * The content canvas: one rounded, elevated surface floating on the field.
   * Everything else in the app lives inside it.
   */
  .canvas {
    position: relative;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    border-radius: var(--canvas-radius);
    background: var(--surface);
    box-shadow: var(--shadow-canvas);
  }
</style>
