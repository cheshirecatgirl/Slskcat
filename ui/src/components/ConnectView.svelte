<script lang="ts">
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import type { Config } from "../lib/types";

  let username = $state("");
  let password = $state("");

  const canSubmit = $derived(username.trim().length > 0 && password.length > 0 && !app.connecting);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canSubmit) return;

    app.connecting = true;
    app.loginError = null;

    // downloadDir and sharedDirs are left to the core's defaults here; both
    // are editable in Settings once there is a session to apply them to.
    const config: Config = {
      credentials: { username: username.trim(), password },
      downloadDir: "",
      sharedDirs: [],
      uploadSlots: 2,
      searchTimeout: 12,
    };

    try {
      await core.connect(config);
    } catch (error) {
      app.connecting = false;
      app.loginError = String(error);
    }
  }
</script>

<div class="wrap">
  <form onsubmit={submit}>
    <div class="mark" aria-hidden="true"></div>
    <h1>Sign in to Soulseek</h1>
    <p class="sub">
      Lark uses your existing Soulseek account. A new username is registered
      automatically the first time you use it.
    </p>

    <label>
      <span>Username</span>
      <input
        class="field"
        bind:value={username}
        autocomplete="username"
        spellcheck="false"
        autocapitalize="off"
        disabled={app.connecting}
      />
    </label>

    <label>
      <span>Password</span>
      <input
        class="field"
        type="password"
        bind:value={password}
        autocomplete="current-password"
        disabled={app.connecting}
      />
    </label>

    {#if app.loginError}
      <p class="error" role="alert">{app.loginError}</p>
    {/if}

    <button class="btn primary" type="submit" disabled={!canSubmit}>
      {app.connecting ? "Connecting…" : "Sign in"}
    </button>
  </form>
</div>

<style>
  .wrap {
    display: grid;
    place-items: center;
    height: 100%;
    padding: 24px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 13px;
    width: 100%;
    max-width: 336px;
  }

  .mark {
    width: 38px;
    height: 38px;
    border-radius: 11px;
    background: linear-gradient(140deg, #4a3a99, #e87997);
    box-shadow: var(--shadow);
  }

  h1 {
    margin-top: 4px;
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.015em;
  }

  .sub {
    margin-top: -6px;
    margin-bottom: 4px;
    color: var(--text-3);
    font-size: 12.5px;
    line-height: 1.55;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  label span {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-2);
  }

  .error {
    padding: 8px 11px;
    border-radius: var(--radius-sm);
    background: var(--danger-quiet);
    color: var(--danger);
    font-size: 12.5px;
  }

  button {
    margin-top: 3px;
    padding: 9px;
  }
</style>
