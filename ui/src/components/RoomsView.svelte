<script lang="ts">
  import { onMount } from "svelte";
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";

  let active = $state<string | null>(null);
  let draft = $state("");
  let query = $state("");
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

  const messages = $derived(active ? (app.roomMessages[active] ?? []) : []);
  const members = $derived(active ? (app.roomUsers[active] ?? []) : []);

  // Follow the conversation as it grows, the way a chat window should.
  $effect(() => {
    void messages.length;
    if (log) log.scrollTop = log.scrollHeight;
  });

  function open(room: string) {
    active = room;
    if (!app.joined.includes(room)) void core.joinRoom(room);
  }

  function leave(room: string) {
    void core.leaveRoom(room);
    if (active === room) active = null;
  }

  function send(event: SubmitEvent) {
    event.preventDefault();
    const body = draft.trim();
    if (!body || !active) return;
    void core.sendRoomMessage(active, body);
    // The server echoes our own line back, so nothing is appended locally.
    draft = "";
  }
</script>

<div class="view">
  <aside>
    <div class="pane-head">
      <input class="field slim" bind:value={query} placeholder="Find a room…" />
    </div>

    {#if app.joined.length > 0}
      <p class="group">Joined</p>
      {#each app.joined as room (room)}
        <div class="room" class:active={active === room}>
          <button class="pick" onclick={() => open(room)}>
            <span class="rname">{room}</span>
          </button>
          <button class="leave btn quiet small" onclick={() => leave(room)} title="Leave">×</button>
        </div>
      {/each}
    {/if}

    <p class="group">All rooms</p>
    {#each listed as room (room.name)}
      <div class="room" class:active={active === room.name}>
        <button class="pick" onclick={() => open(room.name)}>
          <span class="rname">{room.name}</span>
          <span class="rcount num">{room.userCount.toLocaleString()}</span>
        </button>
      </div>
    {:else}
      <p class="none">{app.rooms.length === 0 ? "Loading rooms…" : "No matches."}</p>
    {/each}
  </aside>

  <section>
    {#if !active}
      <div class="empty">
        <h3>No room open</h3>
      </div>
    {:else}
      <header>
        <h2>{active}</h2>
        <span class="num dim">{members.length.toLocaleString()} here</span>
      </header>

      <div class="log" bind:this={log}>
        {#each messages as message, i (i)}
          <div class="line">
            <span class="who">{message.author}</span>
            <span class="body selectable">{message.body}</span>
          </div>
        {:else}
          <p class="none">Nothing said yet.</p>
        {/each}
      </div>

      <form onsubmit={send}>
        <input class="field" bind:value={draft} placeholder="Message {active}…" />
        <button class="btn primary" type="submit" disabled={!draft.trim()}>Send</button>
      </form>
    {/if}
  </section>
</div>

<style>
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
    padding: 11px 10px;
    background: var(--surface-2);
  }
  .field.slim {
    padding: 5px 9px;
    font-size: 12.5px;
  }

  .group {
    padding: 12px 12px 5px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .room {
    display: flex;
    align-items: center;
    margin: 0 6px;
    border-radius: var(--radius-sm);
  }
  .room:hover {
    background: var(--surface-2);
  }
  .room.active {
    background: var(--accent-quiet);
  }
  .room.active .rname {
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
  .rname {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rcount {
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
    padding: 10px 16px;
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
