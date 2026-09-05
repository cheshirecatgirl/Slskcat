<script lang="ts">
  import { player } from "../lib/player.svelte";
  import { duration } from "../lib/format";

  let audio = $state<HTMLAudioElement | null>(null);
  let open = $state(false);

  $effect(() => {
    if (audio) player.attach(audio);
  });

  // Every control writes to the player's fields; this is the one place they
  // reach the element and the graph, so nothing has to remember to apply them.
  $effect(() => {
    void [player.volume, player.speed, player.pitch, player.reverb, player.keepPitch, player.loop];
    player.apply();
  });

  const at = $derived(duration(player.position) === "—" ? "0:00" : duration(player.position));
  const of = $derived(duration(player.duration) === "—" ? "0:00" : duration(player.duration));
</script>

<!-- Lives outside every view, so playback survives moving between them. -->
<div class="player" class:showing={player.track !== null}>
  <!-- `crossorigin` is what makes this audible at all. Files come from the
       asset protocol, which is a different origin from the page, and a Web
       Audio graph fed by a cross-origin element it could not read with CORS
       outputs silence — the track plays, the clock runs, nothing comes out.
       The asset protocol answers with the page's own origin, so asking for
       CORS is all it takes. It has to be set before `src` is, which is why it
       is here rather than in `attach`. -->
  <audio
    bind:this={audio}
    crossorigin="anonymous"
    ontimeupdate={() => (player.position = audio?.currentTime ?? 0)}
    onloadedmetadata={() => (player.duration = audio?.duration ?? 0)}
    onended={() => player.ended()}
    onpause={() => (player.playing = false)}
    onplay={() => (player.playing = true)}
  ></audio>

  {#if player.track}
    <button
      class="play"
      onclick={() => player.toggle()}
      title={player.playing ? "Pause" : "Play"}
      aria-label={player.playing ? "Pause" : "Play"}
    >
      {#if player.playing}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h4v14H7V5Zm6 0h4v14h-4V5Z" /></svg>
      {:else}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5l11 7-11 7V5Z" /></svg>
      {/if}
    </button>

    <div class="what">
      <span class="name selectable">{player.track.name}</span>
      {#if player.track.from}<span class="from">{player.track.from}</span>{/if}
    </div>

    <span class="num time">{at}</span>
    <input
      class="seek"
      type="range"
      min="0"
      max={player.duration || 0}
      step="0.1"
      value={player.position}
      oninput={(event) => player.seek(Number(event.currentTarget.value))}
      aria-label="Seek"
    />
    <span class="num time">{of}</span>

    <button
      class="loop"
      class:on={player.loop !== "off"}
      onclick={() => player.cycleLoop()}
      title={player.loop === "off"
        ? "Loop this track"
        : player.loop === "track"
          ? "Loop the folder"
          : "Stop looping"}
      aria-label="Loop"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M7 6h10v3l4-4-4-4v3H5v6h2V6Zm10 12H7v-3l-4 4 4 4v-3h12v-6h-2v4Z"
        />
      </svg>
      {#if player.loop === "folder"}<span class="mode num">folder</span>{/if}
    </button>

    <button
      class="fx"
      class:on={open}
      onclick={() => (open = !open)}
      title="Playback controls"
      aria-expanded={open}>FX</button
    >

    <div class="volume">
      <input
        class="vol"
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={player.volume}
        oninput={(event) => (player.volume = Number(event.currentTarget.value))}
        aria-label="Volume"
      />
      <span class="num pct">{Math.round(player.volume * 100)}%</span>
    </div>

    <button class="shut" onclick={() => player.stop()} title="Close" aria-label="Close player"
      >×</button
    >

    {#if open}
      <div class="panel">
        <label>
          <span>Speed <em class="num">{player.speed.toFixed(2)}×</em></span>
          <input
            type="range"
            min="0.5"
            max="2"
            step="0.05"
            value={player.speed}
            oninput={(event) => (player.speed = Number(event.currentTarget.value))}
          />
        </label>

        <label class="check">
          <input type="checkbox" bind:checked={player.keepPitch} />
          <span>Keep pitch while changing speed</span>
        </label>
        <p class="note">
          {player.keepPitch
            ? "Tempo alone, time-stretched. Past about 1.5x that starts to smear."
            : "Pitch rises and falls with speed, the way a record does. Nothing is processed."}
        </p>

        <label>
          <span>
            Pitch
            <em class="num">{player.pitch > 0 ? "+" : ""}{player.pitch.toFixed(1)}</em>
          </span>
          <input
            type="range"
            min="-12"
            max="12"
            step="0.5"
            value={player.pitch}
            disabled={!player.pitchAvailable || !player.effectsAvailable}
            oninput={(event) => (player.pitch = Number(event.currentTarget.value))}
          />
        </label>
        <p class="note">
          {!player.effectsAvailable
            ? "Effects are unavailable here, so this is plain playback."
            : player.pitchAvailable
              ? "Semitones, independent of speed. Both move while the track plays."
              : "Pitch needs an audio worklet, which this platform does not provide."}
        </p>

        <label>
          <span>Reverb <em class="num">{Math.round(player.reverb * 100)}%</em></span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={player.reverb}
            disabled={!player.effectsAvailable}
            oninput={(event) => (player.reverb = Number(event.currentTarget.value))}
          />
        </label>
        <button
          class="reset"
          onclick={() => {
            player.speed = 1;
            player.pitch = 0;
            player.reverb = 0;
          }}>Reset</button
        >
      </div>
    {/if}
  {/if}
</div>

<style>
  .player {
    display: none;
  }
  .player.showing {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    grid-column: 1 / -1;
    /* The window has no left padding, since the sidebar sits flush against
       the edge, so the bar supplies its own. */
    margin: var(--gap) 0 0 var(--gap);
    padding: 8px 12px;
    border-radius: var(--radius);
    background: var(--surface-1);
    border: 1px solid var(--line-soft);
    box-shadow: var(--shadow);
  }

  .play {
    display: grid;
    place-items: center;
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    transition: background var(--fast);
  }
  .play:hover {
    background: var(--accent-hover);
  }
  .play svg {
    width: 14px;
    height: 14px;
    fill: currentColor;
  }

  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    width: 200px;
    line-height: 1.25;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-weight: 500;
  }
  .from {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10.5px;
    color: var(--text-3);
  }

  .time {
    flex: none;
    width: 44px;
    font-size: 11.5px;
    color: var(--text-3);
    text-align: center;
  }
  .seek {
    flex: 1;
    min-width: 0;
  }
  .vol {
    flex: none;
    width: 84px;
  }

  .volume {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* Wide enough for "100%" so the row does not shift as the number changes. */
  .pct {
    width: 34px;
    font-size: 11.5px;
    color: var(--text-3);
    text-align: right;
  }

  .loop {
    flex: none;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 7px;
    border-radius: 6px;
    color: var(--text-3);
    transition: background var(--fast), color var(--fast);
  }
  .loop svg {
    display: block;
    width: 15px;
    height: 15px;
    fill: currentColor;
  }
  .loop:hover {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .loop.on {
    color: var(--accent);
  }
  .mode {
    font-size: 10.5px;
    letter-spacing: 0.02em;
  }

  .fx,
  .shut {
    flex: none;
    padding: 4px 8px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-3);
    transition: background var(--fast), color var(--fast);
  }
  /* The close glyph carries its own size: at 11.5px a multiplication sign
     reads as a speck beside the FX label. */
  .shut {
    font-size: 15px;
    line-height: 1;
  }
  .fx:hover,
  .shut:hover,
  .fx.on {
    background: var(--surface-3);
    color: var(--text-1);
  }

  /* Opens upward: the bar is already at the bottom of the window. */
  .panel {
    position: absolute;
    right: 10px;
    bottom: calc(100% + 8px);
    display: grid;
    gap: 11px;
    width: 280px;
    padding: 13px;
    border-radius: var(--radius);
    border: 1px solid var(--line-soft);
    background: var(--surface-1);
    box-shadow: var(--shadow);
  }
  .panel label {
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
  }
  .panel label em {
    font-style: normal;
    color: var(--text-1);
  }
  /* The other labels stack a name over its slider, and `.panel label` sets
     `display: grid` to do it, so this one has to set `display` back. */
  .panel .check {
    display: flex;
  }
  .reset {
    justify-self: start;
    padding: 4px 9px;
    border-radius: 6px;
    background: var(--surface-2);
    font-size: 11.5px;
    color: var(--text-2);
    transition: background var(--fast), color var(--fast);
  }
  .reset:hover {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .panel input:disabled {
    opacity: 0.45;
  }

  .panel .note {
    margin-top: -5px;
    font-size: 11px;
    line-height: 1.45;
    color: var(--text-3);
  }
</style>
