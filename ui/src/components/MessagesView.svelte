<script lang="ts">
  import { onMount } from "svelte";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";

  /** Which conversation the right-hand pane is showing. */
  type Open = { kind: "room"; name: string } | { kind: "direct"; name: string } | null;

  /**
   * Which list the left pane is showing.
   *
   * Rooms and people were one scrolling column before, so a handful of
   * conversations sat above three hundred rooms and went looking for them
   * every time. They are different things and they are separated now.
   */
  let side = $state<"rooms" | "users">("rooms");

  let open = $state<Open>(null);
  let draft = $state("");
  let query = $state("");
  let newPeer = $state("");
  let log = $state<HTMLDivElement | null>(null);

  onMount(() => {
    void core.requestRoomList();
  });

  const listed = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const rooms = needle
      ? app.rooms.filter((r) => r.name.toLowerCase().includes(needle))
      : app.rooms;
    return [...rooms].sort((a, b) => b.userCount - a.userCount).slice(0, 300);
  });

  const peers = $derived(Object.keys(app.conversations).sort((a, b) => a.localeCompare(b)));

  const messages = $derived.by(() => {
    if (!open) return [];
    return open.kind === "room"
      ? (app.roomMessages[open.name] ?? [])
      : (app.conversations[open.name] ?? []);
  });

  const members = $derived(
    open?.kind === "room" ? (app.roomUsers[open.name] ?? []) : [],
  );

  // Follow the conversation as it grows, the way a chat window should.
  $effect(() => {
    void messages.length;
    if (log) log.scrollTop = log.scrollHeight;
  });

  function openRoom(name: string) {
    open = { kind: "room", name };
    if (!app.joined.includes(name)) void core.joinRoom(name);
  }

  function openDirect(name: string) {
    open = { kind: "direct", name };
    // A message arriving from someone while the rooms list is up would
    // otherwise open a thread the reader cannot see selected anywhere.
    side = "users";
  }

  function leave(name: string) {
    void core.leaveRoom(name);
    if (open?.kind === "room" && open.name === name) open = null;
  }

  function startDirect(event: SubmitEvent) {
    event.preventDefault();
    const name = newPeer.trim();
    if (!name) return;
    // An empty thread is enough to open the pane; the first line creates it.
    if (!app.conversations[name]) app.conversations = { ...app.conversations, [name]: [] };
    openDirect(name);
    newPeer = "";
  }

  function send(event: SubmitEvent) {
    event.preventDefault();
    const body = draft.trim();
    if (!body || !open) return;

    if (open.kind === "room") {
      // The server echoes our own room lines back, so nothing is added here.
      void core.sendRoomMessage(open.name, body);
    } else {
      void core.sendPrivateMessage(open.name, body);
      // Private messages are not echoed, so the sent line is recorded locally.
      app.addMessage(open.name, { author: app.username, body });
    }
    draft = "";
  }
</script>

<div class="view">
  <aside>
    <div class="pane-head">
      <div class="seg" role="tablist">
        <button
          class="segbtn"
          class:on={side === "rooms"}
          role="tab"
          aria-selected={side === "rooms"}
          onclick={() => (side = "rooms")}
        >
          Rooms
        </button>
        <button
          class="segbtn"
          class:on={side === "users"}
          role="tab"
          aria-selected={side === "users"}
          onclick={() => (side = "users")}
        >
          Users
          {#if peers.length > 0}<span class="pip num">{peers.length}</span>{/if}
        </button>
      </div>
    </div>

    {#if side === "users"}
      <form class="newpeer" onsubmit={startDirect}>
        <input class="field slim" bind:value={newPeer} placeholder="Message a user…" />
      </form>

      {#each peers as peer (peer)}
        <div class="entry" class:active={open?.kind === "direct" && open.name === peer}>
          <button class="pick" onclick={() => openDirect(peer)}>
            <span class="name">{peer}</span>
          </button>
        </div>
      {:else}
        <p class="none">No conversations yet.</p>
      {/each}
    {:else}
      <div class="pane-head">
        <input class="field slim" bind:value={query} placeholder="Find a room…" />
      </div>

      {#if app.joined.length > 0}
        <p class="group">Joined</p>
        {#each app.joined as room (room)}
          <div class="entry" class:active={open?.kind === "room" && open.name === room}>
            <button class="pick" onclick={() => openRoom(room)}>
              <span class="name">{room}</span>
            </button>
            <button class="leave btn quiet small" onclick={() => leave(room)} title="Leave">×</button
            >
          </div>
        {/each}
      {/if}

      <p class="group">All rooms</p>
      {#each listed as room (room.name)}
        <div class="entry" class:active={open?.kind === "room" && open.name === room.name}>
          <button class="pick" onclick={() => openRoom(room.name)}>
            <span class="name">{room.name}</span>
            <span class="count num">{room.userCount.toLocaleString()}</span>
          </button>
        </div>
      {:else}
        <p class="none">{app.rooms.length === 0 ? "Loading rooms…" : "No matches."}</p>
      {/each}
    {/if}
  </aside>

  <section>
    {#if !open}
      <div class="empty"><h3>Nothing open</h3></div>
    {:else}
      <header>
        <h2>{open.name}</h2>
        {#if open.kind === "room"}
          <span class="num dim">{members.length.toLocaleString()} here</span>
        {:else}
          <span class="tag">direct</span>
        {/if}
      </header>

      <div class="log" bind:this={log}>
        {#each messages as message, i (i)}
          <div class="line" class:mine={message.author === app.username}>
            <span class="who">{message.author}</span>
            <span class="body selectable">{message.body}</span>
          </div>
        {:else}
          <p class="none">Nothing said yet.</p>
        {/each}
      </div>

      <form onsubmit={send}>
        <input class="field" bind:value={draft} placeholder="Message {open.name}…" />
        <button class="btn primary" type="submit" disabled={!draft.trim()}>Send</button>
      </form>
    {/if}
  </section>
</div>

<style>
  /* Rooms and people are different lists, so the pane switches between them
     rather than stacking one above the other. */
  .seg {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-sm);
    /* The pane behind this is itself `--surface-2`, so the track has to step
       away from it or the control reads as two words of plain text. Same
       reason `.field` is overridden a few rules down. */
    background: var(--surface);
  }
  .segbtn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    flex: 1;
    padding: 4px 10px;
    border-radius: calc(var(--radius-sm) - 2px);
    font-size: 12px;
    color: var(--text-3);
    transition: background var(--fast), color var(--fast);
  }
  .segbtn.on {
    background: var(--surface-1);
    color: var(--text-1);
  }
  .segbtn .pip {
    padding: 0 5px;
    border-radius: 999px;
    background: var(--accent-quiet);
    color: var(--accent);
    font-size: 10.5px;
  }

  .view {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    height: 100%;
  }

  aside {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    background: var(--surface-2);
  }
  .pane-head {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 11px 10px;
    background: var(--surface-2);
  }
  /* The panel is itself `--surface-2`, which is what `.field` uses, so an
     unmodified input would be invisible against it. */
  .field.slim {
    padding: 5px 9px;
    font-size: 12.5px;
    background: var(--surface);
    border-color: var(--line-soft);
  }
  .field.slim:focus {
    border-color: var(--accent);
  }
  .newpeer {
    padding: 4px 10px 2px;
  }

  .group {
    padding: 12px 12px 5px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .entry {
    display: flex;
    align-items: center;
    margin: 0 6px;
    border-radius: var(--radius-sm);
    transition: background var(--fast);
  }
  .entry:hover {
    background: #ffffff0a;
  }
  .entry.active {
    background: var(--accent-quiet);
  }
  .entry.active .name {
    color: var(--accent);
    font-weight: 500;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 6px 8px;
    text-align: left;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 11px;
    color: var(--text-3);
  }
  .leave {
    padding: 2px 7px;
    font-size: 14px;
    line-height: 1;
  }
  .none {
    padding: 10px 12px;
    font-size: 12px;
    color: var(--text-3);
  }

  section {
    display: flex;
    flex-direction: column;
    min-width: 0;
    height: 100%;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 15px 18px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }
  .dim {
    font-size: 12px;
    color: var(--text-3);
  }

  .log {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 18px;
  }
  .line {
    display: grid;
    grid-template-columns: 128px minmax(0, 1fr);
    gap: 12px;
    padding: 2px 0;
    font-size: 12.5px;
    line-height: 1.5;
  }
  .who {
    color: var(--accent);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
  }
  /* Own lines are muted rather than highlighted: the eye should land on what
     other people said. */
  .line.mine .who {
    color: var(--text-3);
  }
  .body {
    overflow-wrap: anywhere;
  }

  form {
    display: flex;
    gap: 8px;
    padding: 11px 18px 14px;
  }
  form .btn {
    flex: none;
  }
</style>
