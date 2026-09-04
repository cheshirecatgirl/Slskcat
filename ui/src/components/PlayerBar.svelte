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
    void [player.volume, player.speed, player.pitch, player.reverb];
    player.apply();
  });

  const at = $derived(duration(player.position) === "—" ? "0:00" : duration(player.position));
  const of = $derived(duration(player.duration) === "—" ? "0:00" : duration(player.duration));
</script>

<!-- Lives outside every view, so playback survives moving between them. -->
<div class="bar" class:showing={player.track !== null}>
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
    onended={() => (player.playing = false)}
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
      class="fx"
      class:on={open}
      onclick={() => (open = !open)}
      title="Playback controls"
      aria-expanded={open}>FX</button
    >

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
  .bar {
    display: none;
  }
  .bar.showing {
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
  .fx:hover,
  .shut:hover,
  .fx.on {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .shut {
    font-size: 15px;
    line-height: 1;
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
