<script lang="ts">
  import { onMount } from "svelte";
  import { core, onEvent } from "./lib/core";
  import { app, AppState, fire} from "./lib/state.svelte";
  import type { Section } from "./lib/nav";
  import Sidebar from "./components/Sidebar.svelte";
  import Notices from "./components/Notices.svelte";
  import CommandBar from "./components/CommandBar.svelte";
  import ConnectView from "./components/ConnectView.svelte";
  import SearchView from "./components/SearchView.svelte";
  import WishlistView from "./components/WishlistView.svelte";
  import TransfersView from "./components/TransfersView.svelte";
  import BrowseView from "./components/BrowseView.svelte";
  import LibraryView from "./components/LibraryView.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import MessagesView from "./components/MessagesView.svelte";
  import SettingsView from "./components/SettingsView.svelte";

  let section = $state<Section>("explore");
  let palette = $state(false);

  onMount(() => {
    // Preferences first: the sign-in form and the settings screen both render
    // from them, and a failed load must be visible rather than silently
    // resetting what the user configured.
    core
      .loadSettings()
      .then(async (loaded) => {
        app.settings = loaded;
        // A remembered password is a standing instruction to sign in. Making
        // someone confirm it every launch asks them to re-approve a decision
        // they already made, and the form they would be confirming is one
        // they cannot usefully change — it is already filled in.
        if (!loaded.rememberPassword || !loaded.username || !loaded.password) return;
        app.connecting = true;
        app.resuming = true;
        try {
          app.settings = await core.connect(loaded);
        } catch (error) {
          // Falls through to the form with the reason on it, which is the
          // same place a failed manual sign-in lands.
          app.connecting = false;
          app.resuming = false;
          app.loginError = String(error);
        }
      })
      .catch((error) => {
        app.settingsError = String(error);
        app.notify(`Could not load settings: ${error}`, "danger");
      });

    void refreshDownloaded();

    // The subscription resolves asynchronously; unsubscribing has to wait for
    // it, so the cleanup awaits the same promise rather than racing it.
    const subscription = onEvent((event) => {
      app.apply(event);
      // Rooms are rejoined on sign-in because the server remembers none
      // between sessions: without this the remembered list would sit there
      // named but silent.
      if (event.type === "connected") {
        // Recorded here rather than on save: an account only belongs in the
        // switcher once the server has accepted it, or a mistyped name is
        // offered forever as somewhere to switch to.
        const who = event.data.username;
        void core
          .rememberAccount(who)
          .then((saved) => (app.settings = saved))
          .catch(() => {});
        for (const room of app.settings?.rooms ?? []) fire(core.joinRoom(room));
      }
      // A finished transfer is the one thing that changes what is on disk
      // while the app is running, so it is the only thing that re-reads it.
      if (event.type === "transferUpdated" && event.data.state.type === "completed") {
        void refreshDownloaded();
      }
    });
    return () => {
      void subscription.then((unlisten) => unlisten());
    };
  });

  /** Re-read the download folder. Failure is silent: not knowing what is on
      disk costs a hint, and is not worth a notice over. */
  async function refreshDownloaded() {
    try {
      const files = await core.downloadedFiles();
      app.downloaded = new Set(files.map((file) => AppState.had(file.name, file.size)));
    } catch {
      app.downloaded = new Set();
    }
  }

  // Applied to the document rather than passed down: every size in this
  // interface is in px, so scaling has to happen above all of them. `zoom`
  // reflows rather than transforming, which keeps text crisp and hit areas
  // where they look.
  $effect(() => {
    const scale = app.settings?.uiScale ?? 100;
    document.documentElement.style.zoom = `${scale}%`;
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
<div class="window" data-space={app.connected && !app.addingAccount ? section : "explore"}>
  <div class="field-glow" aria-hidden="true"></div>

  {#if app.connected && !app.addingAccount}
    <Sidebar bind:section onCommand={() => (palette = true)} />
  {/if}

  <div class="canvas" class:solo={!app.connected || app.addingAccount}>
    {#if !app.connected || app.addingAccount}
      <ConnectView />
    {:else if section === "explore"}
      <SearchView />
    {:else if section === "wishlist"}
      <WishlistView />
    {:else if section === "transfers"}
      <TransfersView />
    {:else if section === "discover"}
      <BrowseView />
    {:else if section === "library"}
      <LibraryView />
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

  <!-- Outside the canvas, so what is playing survives moving between
       sections and stays put at the bottom of the window. -->
  {#if app.connected && !app.addingAccount}
    <PlayerBar />
  {/if}

  <Notices />
</div>

<style>
  .window {
    position: relative;
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    /* A second row for the player, which takes only the height it needs and
       leaves the rest to the content above it. */
    grid-template-rows: minmax(0, 1fr) auto;
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
