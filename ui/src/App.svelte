<script lang="ts">
  import { onMount } from "svelte";
  import { onEvent } from "./lib/core";
  import { app } from "./lib/state.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Notices from "./components/Notices.svelte";
  import ConnectView from "./components/ConnectView.svelte";
  import SearchView from "./components/SearchView.svelte";
  import TransfersView from "./components/TransfersView.svelte";
  import BrowseView from "./components/BrowseView.svelte";
  import RoomsView from "./components/RoomsView.svelte";
  import SettingsView from "./components/SettingsView.svelte";
  import type { Section } from "./lib/nav";

  let section = $state<Section>("search");

  onMount(() => {
    // The subscription resolves asynchronously; unsubscribing has to wait for
    // it, so the cleanup awaits the same promise rather than racing it.
    const subscription = onEvent((event) => app.apply(event));
    return () => {
      void subscription.then((unlisten) => unlisten());
    };
  });
</script>

<div class="shell">
  <Sidebar bind:section />

  <main>
    {#if !app.connected}
      <ConnectView />
    {:else if section === "search"}
      <SearchView />
    {:else if section === "transfers"}
      <TransfersView />
    {:else if section === "browse"}
      <BrowseView />
    {:else if section === "rooms"}
      <RoomsView />
    {:else}
      <SettingsView />
    {/if}
  </main>

  <Notices />
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-w) 1fr;
    height: 100%;
  }

  main {
    position: relative;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    background: var(--surface);
    border-left: 1px solid var(--line);
  }
</style>
