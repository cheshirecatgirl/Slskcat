<script lang="ts">
  import { core } from "../lib/core";
  import { app } from "../lib/state.svelte";
  import { defaultSettings } from "../lib/types";

  let username = $state("");
  let password = $state("");
  let remember = $state(false);
  /** True once the stored settings have been copied into the fields. */
  let prefilled = $state(false);
  /** True once anything has been typed, which prefilling must not overwrite. */
  let touched = $state(false);

  // Settings load asynchronously, so the form fills itself in when they
  // arrive rather than rendering blank and staying that way. Someone who
  // starts typing before that resolves keeps what they typed: on a cold start
  // the load can easily lose the race, and having the username vanish
  // mid-word is worse than not prefilling at all.
  $effect(() => {
    const stored = app.settings;
    if (!stored || prefilled || touched) return;
    username = stored.username;
    password = stored.password;
    remember = stored.rememberPassword;
    prefilled = true;
  });

  const canSubmit = $derived(
    username.trim().length > 0 && password.length > 0 && !app.connecting,
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canSubmit) return;

    app.connecting = true;
    app.loginError = null;

    try {
      // The backend persists what it was given, so the stored settings always
      // describe a configuration that was actually used to sign in.
      const saved = await core.connect({
        ...(app.settings ?? defaultSettings()),
        username: username.trim(),
        password,
        rememberPassword: remember,
      });
      app.settings = saved;

      if (remember && !saved.keychainAvailable) {
        app.notify("Signed in. Password not saved: no keychain available.");
      }
    } catch (error) {
      app.connecting = false;
      app.loginError = String(error);
    }
  }
</script>

<div class="wrap">
  <form onsubmit={submit}>
    <div class="mark" aria-hidden="true"></div>
    <h1>Sign in</h1>

    <label>
      <span>Username</span>
      <input
        class="field"
        bind:value={username}
        oninput={() => (touched = true)}
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
        oninput={() => (touched = true)}
        autocomplete="current-password"
        disabled={app.connecting}
      />
    </label>

    <label class="check">
      <input type="checkbox" bind:checked={remember} disabled={app.connecting} />
      <span>Remember password</span>
    </label>
    <p class="note">Stored in the system keychain, not on disk.</p>

    {#if app.loginError}
      <p class="error" role="alert">{app.loginError}</p>
    {/if}
    {#if app.settingsError}
      <p class="error" role="alert">{app.settingsError}</p>
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
    max-width: 348px;
    animation: rise var(--spring) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }

  .mark {
    width: 42px;
    height: 42px;
    border-radius: 13px;
    background: linear-gradient(140deg, var(--accent), var(--accent-hover));
    box-shadow: 0 6px 22px -6px var(--accent);
  }

  h1 {
    margin-top: 4px;
    margin-bottom: 3px;
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.015em;
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

  .check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    margin-top: -3px;
    cursor: pointer;
  }
  .check input {
    accent-color: var(--accent);
  }
  .check span {
    font-weight: 400;
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

  .note {
    margin-top: -8px;
    padding-left: 21px;
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
