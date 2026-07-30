<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { core } from "../lib/core";
  import type { Section } from "../lib/nav";

  let {
    section = $bindable(),
    onCommand,
  }: { section: Section; onCommand: () => void } = $props();

  // Inline SVG rather than an icon package: a handful of paths costs nothing
  // and keeps the bundle free of a dependency that would dwarf them.
  const items: { id: Section; label: string; path: string }[] = [
    {
      id: "search",
      label: "Search",
      path: "M11 4a7 7 0 1 0 4.2 12.6l3.6 3.6 1.4-1.4-3.6-3.6A7 7 0 0 0 11 4Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z",
    },
    {
      id: "wishlist",
      label: "Wishlist",
      path: "M12 21s-7.5-4.7-9.3-9A5.6 5.6 0 0 1 12 6.2 5.6 5.6 0 0 1 21.3 12c-1.8 4.3-9.3 9-9.3 9Zm0-2.5c2-1.4 5.6-4.2 7-7.3a3.6 3.6 0 0 0-6-2.6L12 9.7l-1-1.1a3.6 3.6 0 0 0-6 2.6c1.4 3.1 5 5.9 7 7.3Z",
    },
    {
      id: "transfers",
      label: "Transfers",
      path: "M12 3v12.2l4.6-4.6 1.4 1.4-7 7-7-7 1.4-1.4 4.6 4.6V3h2ZM4 19h16v2H4v-2Z",
    },
    {
      id: "browse",
      label: "Browse",
      path: "M4 5a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5Zm2 2v10h12V7H6Z",
    },
    {
      id: "rooms",
      label: "Rooms",
      path: "M4 4h16a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H9l-5 4V6a2 2 0 0 1 2-2Zm0 2v10.9L8.3 15H20V6H4Z",
    },
    {
      id: "settings",
      label: "Settings",
      path: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm0 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm-1.2-8h2.4l.4 2.3 2 .9 1.9-1.3 1.7 1.7-1.3 1.9.9 2 2.3.4v2.4l-2.3.4-.9 2 1.3 1.9-1.7 1.7-1.9-1.3-2 .9-.4 2.3h-2.4l-.4-2.3-2-.9-1.9 1.3-1.7-1.7 1.3-1.9-.9-2L2 13.2v-2.4l2.3-.4.9-2L3.9 6.5l1.7-1.7 1.9 1.3 2-.9L10.8 2Z",
    },
  ];
</script>

<aside>
  <div class="brand">
    <div class="mark" aria-hidden="true"></div>
    <span>slsk.cat</span>
  </div>

  <!-- The command bar is the primary way in, so it sits above navigation. -->
  <button class="command" onclick={onCommand}>
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M11 4a7 7 0 1 0 4.2 12.6l3.6 3.6 1.4-1.4-3.6-3.6A7 7 0 0 0 11 4Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z"
      />
    </svg>
    <span>Search…</span>
    <span class="kbd">⌘K</span>
  </button>

  <nav>
    {#each items as item (item.id)}
      <button
        class="item"
        class:active={section === item.id}
        onclick={() => (section = item.id)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.path} /></svg>
        <span class="label">{item.label}</span>
        {#if item.id === "transfers" && app.activeWork > 0}
          <span class="badge num">{app.activeWork}</span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="foot">
    <div class="who">
      <span class="dot" aria-hidden="true"></span>
      <span class="name selectable" title={app.username}>{app.username}</span>
      <button class="out" onclick={() => core.disconnect()} title="Sign out" aria-label="Sign out">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M10 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h5v-2H5V5h5V3Zm6.2 4.8-1.4 1.4L16.6 11H8v2h8.6l-1.8 1.8 1.4 1.4L20.4 12l-4.2-4.2Z" />
        </svg>
      </button>
    </div>
    {#if app.shares}
      <p class="shares num">{app.shares.files.toLocaleString()} files shared</p>
    {/if}
  </div>
</aside>

<style>
  /* No panel and no border: the sidebar sits directly on the coloured field. */
  aside {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 12px 10px 12px 12px;
    position: relative;
    z-index: 1;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 3px 6px 13px;
    font-weight: 600;
    font-size: 14px;
    letter-spacing: -0.015em;
  }
  .mark {
    width: 19px;
    height: 19px;
    border-radius: 6px;
    background: linear-gradient(140deg, var(--accent), var(--accent-hover));
    box-shadow: 0 2px 8px -2px var(--accent);
    transition: background var(--slow), box-shadow var(--slow);
  }

  .command {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 9px;
    margin-bottom: 14px;
    border-radius: var(--radius);
    background: #ffffff0f;
    color: var(--text-2);
    font-size: 12.5px;
    text-align: left;
    transition: background var(--fast), transform var(--fast);
  }
  .command:hover {
    background: #ffffff1a;
    color: var(--text);
  }
  .command:active {
    transform: scale(0.98);
  }
  .command svg {
    width: 14px;
    height: 14px;
    flex: none;
    fill: currentColor;
    opacity: 0.7;
  }
  .command span:not(.kbd) {
    flex: 1;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 9px;
    border-radius: var(--radius);
    color: var(--text-2);
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    transition: background var(--fast), color var(--fast), transform var(--fast);
  }
  .item:hover:not(.active) {
    background: #ffffff0f;
    color: var(--text);
  }
  .item:active {
    transform: scale(0.98);
  }
  /* The active pill is solid and accent-tinted, so the sidebar always says
     which space you are in even at a glance. */
  .item.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px #00000040;
  }
  .item svg {
    width: 16px;
    height: 16px;
    flex: none;
    fill: currentColor;
    opacity: 0.85;
  }
  .item.active svg {
    fill: var(--accent);
    opacity: 1;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
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
    padding: 8px 4px 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 999px;
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 22%, transparent);
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .out {
    flex: none;
    padding: 3px;
    border-radius: 6px;
    color: var(--text-3);
    transition: color var(--fast), background var(--fast);
  }
  .out:hover {
    background: #ffffff14;
    color: var(--text);
  }
  .out svg {
    width: 14px;
    height: 14px;
    display: block;
    fill: currentColor;
  }
  .shares {
    padding-left: 14px;
    font-size: 11px;
    color: var(--text-3);
  }
</style>
