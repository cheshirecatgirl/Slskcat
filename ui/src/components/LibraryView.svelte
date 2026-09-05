<script lang="ts">
  import { app } from "../lib/state.svelte";
  import LocalFilesView from "./LocalFilesView.svelte";
  import WishlistView from "./WishlistView.svelte";

  /**
   * Files here, wishes there.
   *
   * Both are the same question asked at different times: what do I have, and
   * what am I still waiting for. They were two sections in the sidebar saying
   * one thing each.
   */
  let side = $state<"files" | "wishes">("files");

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
        Files
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
  </div>

  <div class="body" role="tabpanel">
    {#if side === "files"}
      <LocalFilesView />
    {:else}
      <WishlistView />
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
    padding: 12px 18px 0;
  }
  /* Narrow, and left of the row: this switches between two lists rather than
     heading the page, so it should not span it. */

  .body {
    flex: 1;
    min-height: 0;
  }
</style>
