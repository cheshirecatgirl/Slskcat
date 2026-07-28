<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { core } from "../lib/core";
  import type { Section } from "../lib/nav";

  let { section = $bindable() }: { section: Section } = $props();

  // Inline SVG rather than an icon package: a handful of paths costs nothing
  // and keeps the bundle free of a dependency that would dwarf them.
  const items: { id: Section; label: string; path: string }[] = [
    { id: "search", label: "Search", path: "M11 4a7 7 0 1 0 4.2 12.6l3.6 3.6 1.4-1.4-3.6-3.6A7 7 0 0 0 11 4Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z" },
    { id: "transfers", label: "Transfers", path: "M12 3v12.2l4.6-4.6 1.4 1.4-7 7-7-7 1.4-1.4 4.6 4.6V3h2ZM4 19h16v2H4v-2Z" },
    { id: "browse", label: "Browse", path: "M4 5a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5Zm2 2v10h12V7H6Z" },
    { id: "rooms", label: "Rooms", path: "M4 4h16a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H9l-5 4V6a2 2 0 0 1 2-2Zm0 2v10.9L8.3 15H20V6H4Z" },
    { id: "settings", label: "Settings", path: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm0 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm-1.2-8h2.4l.4 2.3 2 .9 1.9-1.3 1.7 1.7-1.3 1.9.9 2 2.3.4v2.4l-2.3.4-.9 2 1.3 1.9-1.7 1.7-1.9-1.3-2 .9-.4 2.3h-2.4l-.4-2.3-2-.9-1.9 1.3-1.7-1.7 1.3-1.9-.9-2L2 13.2v-2.4l2.3-.4.9-2L3.9 6.5l1.7-1.7 1.9 1.3 2-.9L10.8 2Z" },
  ];
</script>

<aside>
  <div class="brand">
    <div class="mark" aria-hidden="true"></div>
    <span>Lark</span>
  </div>

  <nav>
    {#each items as item (item.id)}
      <button
        class="item"
        class:active={section === item.id}
        disabled={!app.connected}
        onclick={() => (section = item.id)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.path} /></svg>
        <span>{item.label}</span>
        {#if item.id === "transfers" && app.activeTransfers > 0}
          <span class="badge num">{app.activeTransfers}</span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="foot">
    {#if app.connected}
      <div class="who">
        <span class="dot" aria-hidden="true"></span>
        <span class="name selectable" title={app.username}>{app.username}</span>
      </div>
      {#if app.shares}
        <p class="shares num">
          Sharing {app.shares.files.toLocaleString()} files
        </p>
      {/if}
      <button class="btn quiet small" onclick={() => core.disconnect()}>Sign out</button>
    {:else}
      <div class="who">
        <span class="dot off" aria-hidden="true"></span>
        <span class="name">Offline</span>
      </div>
    {/if}
  </div>
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 14px 10px 12px;
    background: var(--bg);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 4px 8px 16px;
    font-weight: 600;
    font-size: 14px;
    letter-spacing: -0.01em;
  }
  .mark {
    width: 18px;
    height: 18px;
    border-radius: 5px;
    background: linear-gradient(140deg, #4a3a99, #e87997);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 8px;
    border-radius: var(--radius-sm);
    color: var(--text-2);
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    transition: background var(--fast), color var(--fast);
  }
  .item:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }
  .item.active {
    background: var(--surface-3);
    color: var(--text);
  }
  .item:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .item svg {
    width: 16px;
    height: 16px;
    flex: none;
    fill: currentColor;
  }

  .badge {
    margin-left: auto;
    min-width: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 600;
    text-align: center;
  }

  .foot {
    margin-top: auto;
    padding: 10px 8px 2px;
    border-top: 1px solid var(--line-soft);
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    width: 100%;
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 999px;
    background: var(--ok);
  }
  .dot.off {
    background: var(--text-3);
  }
  .name {
    font-size: 12.5px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .shares {
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
