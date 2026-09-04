<script lang="ts">
  import { core } from "../lib/core";
  import { app, fire} from "../lib/state.svelte";
  import type { Section } from "../lib/nav";

  let {
    section = $bindable(),
    onclose,
  }: { section: Section; onclose: () => void } = $props();

  let query = $state("");
  let cursor = $state(0);
  let input = $state<HTMLInputElement | null>(null);

  type Action = {
    id: string;
    label: string;
    hint: string;
    run: () => void;
  };

  const go = (to: Section, label: string, hint: string): Action => ({
    id: `go:${to}`,
    label,
    hint,
    run: () => {
      section = to;
      onclose();
    },
  });

  const navigation: Action[] = [
    go("search", "Search", "Go to search"),
    go("transfers", "Transfers", "Go to transfers"),
    go("browse", "Browse", "Go to browse"),
    go("messages", "Messages", "Go to rooms and direct messages"),
    go("settings", "Settings", "Go to settings"),
  ];

  /**
   * The palette is search-first: typing anything offers to run it as a network
   * search, and matching navigation follows underneath.
   */
  const actions = $derived.by(() => {
    const text = query.trim();
    const needle = text.toLowerCase();

    const matches = needle
      ? navigation.filter((a) => a.label.toLowerCase().includes(needle))
      : navigation;

    if (!text) return matches;

    return [
      {
        id: "search:run",
        label: `Search for “${text}”`,
        hint: "Search the network",
        run: () => {
          void start(text);
          onclose();
        },
      },
      {
        id: "browse:run",
        label: `Browse ${text}`,
        hint: "Open this user's shares",
        run: () => {
          fire(core.browseUser(text));
          section = "browse";
          onclose();
        },
      },
      ...matches,
    ];
  });

  // Keep the cursor inside the list as it shrinks under a longer query.
  $effect(() => {
    if (cursor >= actions.length) cursor = Math.max(0, actions.length - 1);
  });

  $effect(() => {
    input?.focus();
  });

  async function start(text: string) {
    try {
      const id = await core.search(text);
      app.startSearch(id, text);
      section = "search";
    } catch (error) {
      app.notify(String(error), "danger");
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      cursor = (cursor + 1) % Math.max(1, actions.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      cursor = (cursor - 1 + actions.length) % Math.max(1, actions.length);
    } else if (event.key === "Enter") {
      event.preventDefault();
      actions[cursor]?.run();
    }
  }
</script>

<!-- Keys are bound at the window so they work wherever focus happens to be. -->
<svelte:window onkeydown={onKey} />

<div class="scrim">
  <!-- A real button rather than a click handler on the backdrop div, so
       dismissing is reachable by keyboard and announced properly. -->
  <button class="backdrop" onclick={onclose} aria-label="Close command bar"></button>

  <div class="panel" role="dialog" aria-modal="true" aria-label="Command bar">
    <div class="entry">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M11 4a7 7 0 1 0 4.2 12.6l3.6 3.6 1.4-1.4-3.6-3.6A7 7 0 0 0 11 4Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z"
        />
      </svg>
      <!-- No keydown handler here: the window binding above already sees these
           events as they bubble, and handling both would run every action
           twice. -->
      <input
        bind:this={input}
        bind:value={query}
        placeholder="Search, or jump to…"
        spellcheck="false"
        autocapitalize="off"
      />
      <span class="kbd">esc</span>
    </div>

    {#if actions.length > 0}
      <ul>
        {#each actions as action, i (action.id)}
          <li>
            <button
              class="action"
              class:on={i === cursor}
              onmouseenter={() => (cursor = i)}
              onclick={action.run}
            >
              <span class="label">{action.label}</span>
              <span class="hint">{action.hint}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 60;
    display: flex;
    justify-content: center;
    padding-top: 14vh;
    animation: fade var(--fast) both;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    cursor: default;
    background: #00000059;
    /* The blur is what makes the palette feel like it is floating above the
       app rather than replacing it. */
    backdrop-filter: blur(6px);
  }

  .panel {
    position: relative;
    width: min(560px, calc(100% - 48px));
    height: fit-content;
    max-height: 62vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    animation: drop var(--spring) both;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-12px) scale(0.97);
    }
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 15px 17px;
  }
  .entry svg {
    width: 17px;
    height: 17px;
    flex: none;
    fill: var(--text-3);
  }
  .entry input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
    font-size: 15.5px;
    letter-spacing: -0.01em;
  }
  .entry input::placeholder {
    color: var(--text-3);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0 8px 8px;
    overflow-y: auto;
  }

  .action {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    padding: 8px 11px;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background var(--fast);
  }
  .action.on {
    background: var(--accent-quiet);
  }
  .label {
    flex: 1;
    min-width: 0;
    font-size: 13.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .action.on .label {
    color: var(--accent);
    font-weight: 500;
  }
  .hint {
    flex: none;
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
