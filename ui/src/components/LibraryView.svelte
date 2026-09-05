<script lang="ts">
  import { app } from "../lib/state.svelte";
  import MusicView from "./MusicView.svelte";
  import WishlistView from "./WishlistView.svelte";

  /**
   * Files here, wishes there.
   *
   * Both are the same question asked at different times: what do I have, and
   * what am I still waiting for. They were two sections in the sidebar saying
   * one thing each.
   */
  let side = $state<"files" | "wishes">("files");
  /**
   * Songs or the releases they belong to. One scan, two ways of reading it,
   * so it is a toggle beside the list rather than a third tab.
   */
  let shape = $state<"songs" | "albums">("songs");

  const waiting = $derived(app.settings?.wishlist.length ?? 0);
</script>

<div class="view">
  <div class="head">
    <div class="seg" role="tablist" aria-label="Library">
      <button
        class="segbtn"
        class:on={side === "files"}
        role="tab"
        aria-selected={side === "files"}
        onclick={() => (side = "files")}
      >
        My list
      </button>
      <button
        class="segbtn"
        class:on={side === "wishes"}
        role="tab"
        aria-selected={side === "wishes"}
        onclick={() => (side = "wishes")}
      >
        Wishlist
        {#if waiting > 0}<span class="pip num">{waiting}</span>{/if}
      </button>
    </div>

    {#if side === "files"}
      <div class="shape" role="group" aria-label="How to show them">
        <button
          class="shapebtn"
          class:on={shape === "songs"}
          onclick={() => (shape = "songs")}
          title="As songs"
          aria-label="As songs"
          aria-pressed={shape === "songs"}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 5h2v2H4V5Zm4 0h12v2H8V5ZM4 11h2v2H4v-2Zm4 0h12v2H8v-2Zm-4 6h2v2H4v-2Zm4 0h12v2H8v-2Z" />
          </svg>
        </button>
        <button
          class="shapebtn"
          class:on={shape === "albums"}
          onclick={() => (shape = "albums")}
          title="As releases"
          aria-label="As releases"
          aria-pressed={shape === "albums"}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M4 4h7v7H4V4Zm2 2v3h3V6H6Zm7-2h7v7h-7V4Zm2 2v3h3V6h-3ZM4 13h7v7H4v-7Zm2 2v3h3v-3H6Zm7-2h7v7h-7v-7Zm2 2v3h3v-3h-3Z"
            />
          </svg>
        </button>
      </div>
    {/if}
  </div>

  <div class="body" role="tabpanel">
    {#if side === "wishes"}
      <WishlistView />
    {:else}
      <MusicView {shape} />
    {/if}
  </div>
</div>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 18px 0;
  }
  /* Two ways of showing one list, so: icons, together, no track behind them.
     A second segmented control here would read as a second choice of what to
     look at rather than of how. */
  .shape {
    display: flex;
    gap: 2px;
  }
  .shapebtn {
    display: grid;
    place-items: center;
    padding: 5px;
    border-radius: var(--radius-sm);
    color: var(--text-3);
    transition: background var(--fast), color var(--fast);
  }
  .shapebtn svg {
    display: block;
    width: 15px;
    height: 15px;
    fill: currentColor;
  }
  .shapebtn:hover {
    color: var(--text-1);
  }
  .shapebtn.on {
    background: var(--accent-quiet);
    color: var(--accent);
  }
  /* Narrow, and left of the row: this switches between two lists rather than
     heading the page, so it should not span it. */

  .body {
    flex: 1;
    min-height: 0;
  }
</style>
