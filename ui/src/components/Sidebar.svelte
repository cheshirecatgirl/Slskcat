<script lang="ts">
  import { app } from "../lib/state.svelte";
  import * as session from "../lib/session";
  import { MAX_ACCOUNTS } from "../lib/types";
  import { dismiss } from "../lib/dismiss";
  import { core } from "../lib/core";
  import type { Section } from "../lib/nav";

  let {
    section = $bindable(),
    onCommand,
  }: { section: Section; onCommand: () => void } = $props();

  /** Whether the account menu is showing. */
  let switching = $state(false);

  /**
   * Accounts other than the one signed in, most recent first, capped at five.
   *
   * A switcher is for the handful of names someone actually uses; past that it
   * is a list to search rather than a menu to point at. The rest stay in the
   * settings file and come back if one is signed into again.
   */
  const others = $derived(
    (app.settings?.accounts ?? [])
      .filter((name) => name !== app.username)
      .slice(0, MAX_ACCOUNTS - 1),
  );

  /** Whether there is room for one more. */
  const roomToAdd = $derived((app.settings?.accounts ?? []).length < MAX_ACCOUNTS);

  async function switchTo(username: string) {
    switching = false;
    await session.switchTo(username);
  }

  async function forget(username: string) {
    try {
      app.settings = await core.forgetAccount(username);
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  /**
   * Show an empty sign-in form without ending the current session.
   *
   * Signing out to add an account made the form the only thing on screen with
   * no way back: the account you already had was gone, so cancelling meant
   * signing into it again. `addingAccount` leaves the session alone until a
   * new one actually signs in.
   */
  function addAccount() {
    switching = false;
    app.addingAccount = true;
    // Signing in as the new account ends this one, so if it is refused the
    // form needs to know which name it interrupted.
    app.previousAccount = app.username || null;
  }

  // Inline SVG rather than an icon package: a handful of paths costs nothing
  // and keeps the bundle free of a dependency that would dwarf them.
  const items: { id: Section; label: string; path: string }[] = [
    {
      id: "discover",
      // Someone else's shelves rather than a folder of your own: a compass,
      // hollow with the same two-unit wall as the rest of the set. The needle
      // is a rhombus about the exact centre — the old one was two nested
      // parallelograms whose hole sat below and left of the middle.
      label: "Discover",
      path: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16Zm3.4 4.6-2.3 4.5-4.5 2.3 2.3-4.5 4.5-2.3Z",
    },
    {
      id: "library",
      // Your own files: stacked spines.
      label: "Library",
      // Two spines, not three. Three books in a 24-unit box leaves each of
      // them five units wide, and a five-unit book with a two-unit wall has a
      // one-unit hole: at 18px on screen it fills in and reads as a smudge.
      path: "M4 4h7v16H4V4Zm2 2v12h3V6H6Zm7-3h7v17h-7V3Zm2 2v13h3V5h-3Z",
    },
    {
      id: "wishlist",
      label: "Wishlist",
      // The wall was thinner across the two lobes than down the point,
      // because the outer and inner curves were struck from different
      // centres. Both are arcs of the same radius now, offset by two units.
      path: "M12 20.7 3.9 13a5.1 5.1 0 0 1 7.2-7.2l.9.9.9-.9A5.1 5.1 0 0 1 20.1 13L12 20.7Zm0-2.8 6.7-6.4a3.1 3.1 0 0 0-4.4-4.4L12 9.4 9.7 7.1a3.1 3.1 0 0 0-4.4 4.4L12 17.9Z",
    },
    {
      id: "transfers",
      label: "Transfers",
      // The shaft used to span x 12-14 while the baseline spanned 4-20, so
      // the arrow hung a unit off the centre of its own line. Every part of
      // this one is symmetric about x=12.
      path: "M12 17 6 11l1.4-1.4L11 13.2V3h2v10.2l3.6-3.6L18 11ZM4 19h16v2H4v-2Z",
    },
    {
      id: "messages",
      label: "Messages",
      // A speech bubble said "chat"; the section is rooms and people. Drawn
      // hollow with a 2-unit wall, which is what the rest of the set does.
      path: "M12 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm0 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm0 7.5c-3.9 0-7 2.2-7 5V20h14v-2.5c0-2.8-3.1-5-7-5Zm0 2c2.9 0 5 1.5 5 3V18H7v-.5c0-1.5 2.1-3 5-3Z",
    },
    {
      id: "settings",
      label: "Settings",
      // Eight teeth, every one the same width and the same angle apart,
      // struck from one pair of radii about the centre. The old one was a
      // hand-written polygon whose teeth were neither.
      path: "M10 1.8L14 1.8L13.5 4.3L16.3 5.5L17.8 3.4L20.6 6.2L18.5 7.7L19.7 10.5L22.2 10L22.2 14L19.7 13.5L18.5 16.3L20.6 17.8L17.8 20.6L16.3 18.5L13.5 19.7L14 22.2L10 22.2L10.5 19.7L7.7 18.5L6.2 20.6L3.4 17.8L5.5 16.3L4.3 13.5L1.8 14L1.8 10L4.3 10.5L5.5 7.7L3.4 6.2L6.2 3.4L7.7 5.5L10.5 4.3ZM12 8.7a3.3 3.3 0 1 0 0 6.6a3.3 3.3 0 1 0 0-6.6Z",
    },
  ];
</script>

<aside>
  <div class="brand">
    <div class="mark" aria-hidden="true">
      <svg viewBox="0 0 24 24"><path d="M4.6 3.2a1 1 0 0 1 1.3-.1L9.2 5.6a9.6 9.6 0 0 1 5.6 0l3.3-2.5a1 1 0 0 1 1.6.9l-.5 4.4a8 8 0 0 1 1 3.8c0 4.6-3.9 8-8.2 8s-8.2-3.4-8.2-8a8 8 0 0 1 1-3.8l-.5-4.4a1 1 0 0 1 .3-.8Zm2 2.6.3 2.6-.4.6a6 6 0 0 0-.8 3c0 3.4 2.9 6 6.3 6s6.3-2.6 6.3-6a6 6 0 0 0-.8-3l-.4-.6.3-2.6-1.8 1.4-.6-.2a7.7 7.7 0 0 0-5.4 0l-.6.2-1.8-1.4ZM9.4 11a1.1 1.1 0 1 1 0 2.2 1.1 1.1 0 0 1 0-2.2Zm5.2 0a1.1 1.1 0 1 1 0 2.2 1.1 1.1 0 0 1 0-2.2ZM12 14.6c.6 0 1.1.3 1.1.7 0 .5-.5.9-1.1.9s-1.1-.4-1.1-.9c0-.4.5-.7 1.1-.7Z" /></svg>
    </div>
    <span>slsk.cat</span>
  </div>

  <!-- Search is the section and the way into it, in one row. A separate
       "Explore" below this said the same thing twice. -->
  <div class="command" class:active={section === "explore"}>
    <button class="go" onclick={() => (section = "explore")}>
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M11 4a7 7 0 1 0 4.2 12.6l3.6 3.6 1.4-1.4-3.6-3.6A7 7 0 0 0 11 4Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z"
        />
      </svg>
      <span>Search…</span>
    </button>
    <button class="kbd" onclick={onCommand} title="Command bar" aria-label="Open the command bar">
      ⌘K
    </button>
  </div>

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

  <div class="foot" use:dismiss={() => (switching = false)}>
    {#if switching}
      <!-- Anchored above the identity row, the way an account menu sits above
           the person it belongs to. -->
      <div class="accounts" role="menu">
        {#each others as name (name)}
          <div class="acct" role="none">
            <button class="pick" role="menuitem" onclick={() => switchTo(name)}>
              <span class="initial" aria-hidden="true">{name.slice(0, 1).toUpperCase()}</span>
              <span class="aname">{name}</span>
            </button>
            <button
              class="drop"
              role="menuitem"
              title="Forget {name}"
              aria-label="Forget {name}"
              onclick={() => forget(name)}>×</button
            >
          </div>
        {/each}
        {#if roomToAdd}
          <button class="add" role="menuitem" onclick={addAccount}>Add another account</button>
        {:else}
          <!-- Offering to add one here would have quietly pushed the oldest
               name off the end of the list instead. -->
          <p class="full">{MAX_ACCOUNTS} accounts is the limit. Forget one to add another.</p>
        {/if}
      </div>
    {/if}

    <div class="who">
      <button
        class="name"
        title="Switch account"
        aria-haspopup="menu"
        aria-expanded={switching}
        onclick={() => (switching = !switching)}
      >
        <span class="text">{app.username}</span>
        <span class="caret" class:open={switching} aria-hidden="true">▾</span>
      </button>
      <!-- The same label peers get, rather than a coloured dot that says the
           same thing without saying it. -->
      <span class="tag ok">online</span>
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
    display: grid;
    place-items: center;
    width: 19px;
    height: 19px;
    border-radius: 6px;
    background: linear-gradient(140deg, var(--accent), var(--accent-hover));
    box-shadow: 0 2px 8px -2px var(--accent);
    transition: background var(--slow), box-shadow var(--slow);
  }
  .mark svg {
    width: 12px;
    height: 12px;
    fill: #fff;
  }

  .command {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 0 9px 0 0;
    margin-bottom: 14px;
    border-radius: var(--radius);
    background: #ffffff0f;
    color: var(--text-2);
    font-size: 12.5px;
    transition: background var(--fast);
  }
  .command:hover {
    background: #ffffff1a;
    color: var(--text);
  }
  .command.active {
    background: var(--accent-quiet);
    color: var(--accent);
  }
  /* The row is the section; the chip beside it is the command bar. Two
     targets, because a button cannot hold another one. */
  .command .go {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 7px 0 7px 9px;
    color: inherit;
    font: inherit;
    text-align: left;
  }
  .command .go:active {
    transform: scale(0.98);
  }
  .command svg {
    width: 14px;
    height: 14px;
    flex: none;
    fill: currentColor;
    opacity: 0.7;
  }
  .command .kbd {
    flex: none;
    transition: background var(--fast), color var(--fast);
  }
  .command .kbd:hover {
    background: var(--accent);
    color: #fff;
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
  /* The name is the menu trigger now, so it needs a hit area and a hover —
     but it must still read as a name rather than as a control. */
  .name {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
    /* A control, not prose: dragging across the sidebar used to leave the
       name and its arrow highlighted. */
    user-select: none;
    padding: 2px 5px;
    margin-left: -5px;
    border-radius: 6px;
    font-size: 12.5px;
    font-weight: 500;
    text-align: left;
    transition: background var(--fast);
  }
  .name:hover {
    background: #ffffff14;
  }
  .name .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name .caret {
    flex: none;
    font-size: 9px;
    color: var(--text-3);
    transition: transform var(--fast);
  }
  .name .caret.open {
    transform: rotate(180deg);
  }

  /* The account menu. Sits above the identity row rather than below it,
     because the row is already at the bottom of the window. */
  .full {
    padding: 6px 9px;
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--text-3);
  }
  .accounts {
    /* The name below carries a negative margin so its text lines up with the
       navigation, and the pane has to start where that box does or it looks
       inset on the left and flush on the right. */
    margin-left: -5px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-bottom: 7px;
    padding: 4px;
    border-radius: var(--radius);
    border: 1px solid var(--line-soft);
    background: var(--surface-1);
    box-shadow: var(--shadow);
    animation: rise var(--spring) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  .acct {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .acct .pick {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 5px 6px;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background var(--fast);
  }
  .acct .pick:hover {
    background: var(--accent-quiet);
  }
  .initial {
    display: grid;
    place-items: center;
    flex: none;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: 10.5px;
    font-weight: 600;
  }
  .aname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .drop {
    flex: none;
    width: 20px;
    padding: 3px 0;
    border-radius: 6px;
    color: var(--text-3);
    font-size: 13px;
    line-height: 1;
    transition: color var(--fast), background var(--fast);
  }
  .drop:hover {
    background: var(--danger-quiet);
    color: var(--danger);
  }
  .add {
    margin-top: 2px;
    padding: 6px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-2);
    text-align: left;
    transition: background var(--fast), color var(--fast);
  }
  .add:hover {
    background: var(--surface-2);
    color: var(--text-1);
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
    /* Lines up with the username above it, which sits at the foot's own edge. */
    font-size: 11px;
    color: var(--text-3);
  }
</style>
