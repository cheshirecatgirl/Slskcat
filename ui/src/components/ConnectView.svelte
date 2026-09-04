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
  /** Whether the password is shown as text. */
  let reveal = $state(false);

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

  /** Coming back needs a password to come back with. */
  const canResume = $derived(
    (app.settings?.password ?? "").length > 0 && !app.connecting,
  );

  /** Take the account back. The server gives it to whoever logged in last. */
  async function resume() {
    const stored = app.settings;
    if (!stored) return;
    app.connecting = true;
    app.loginError = null;
    try {
      app.settings = await core.connect(stored);
    } catch (error) {
      app.connecting = false;
      app.loginError = String(error);
    }
  }

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
  {#if app.resuming && !app.loginError}
    <!-- Signing in without a form: with a remembered password there is
         nothing to fill in, and flashing an unusable form for the length of a
         handshake reads as a glitch. -->
    <div class="waiting">
      <div class="mark" aria-hidden="true"></div>
      <p>Signing in as <strong>{app.settings?.username ?? ""}</strong>…</p>
    </div>
  {:else}
  <form onsubmit={submit}>
    <div class="mark" aria-hidden="true">
      <!-- A cat, drawn rather than fetched: two ears, two eyes, a nose. -->
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M4.6 3.2a1 1 0 0 1 1.3-.1L9.2 5.6a9.6 9.6 0 0 1 5.6 0l3.3-2.5a1 1 0 0 1 1.6.9l-.5 4.4a8 8 0 0 1 1 3.8c0 4.6-3.9 8-8.2 8s-8.2-3.4-8.2-8a8 8 0 0 1 1-3.8l-.5-4.4a1 1 0 0 1 .3-.8Zm2 2.6.3 2.6-.4.6a6 6 0 0 0-.8 3c0 3.4 2.9 6 6.3 6s6.3-2.6 6.3-6a6 6 0 0 0-.8-3l-.4-.6.3-2.6-1.8 1.4-.6-.2a7.7 7.7 0 0 0-5.4 0l-.6.2-1.8-1.4ZM9.4 11a1.1 1.1 0 1 1 0 2.2 1.1 1.1 0 0 1 0-2.2Zm5.2 0a1.1 1.1 0 1 1 0 2.2 1.1 1.1 0 0 1 0-2.2ZM12 14.6c.6 0 1.1.3 1.1.7 0 .5-.5.9-1.1.9s-1.1-.4-1.1-.9c0-.4.5-.7 1.1-.7Z"
        />
      </svg>
    </div>
    <h1>{app.addingAccount ? "Add an account" : "Sign in"}</h1>

    {#if app.displaced}
      <!-- Not a failure: the password was right and the server simply handed
           the name to a newer session. One button is the whole remedy. -->
      <div class="displaced" role="status">
        <p>Your account was signed in somewhere else.</p>
        <p class="why">The server allows one session per account, so this one was ended.</p>
        <button class="btn primary" type="button" onclick={resume} disabled={!canResume}>
          Go back online
        </button>
      </div>
    {/if}

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
      <div class="secret">
        <!-- `type` is set rather than bound: Svelte refuses `bind:value` on an
             input whose type is dynamic, and one input that changes type keeps
             the caret, the selection and the password manager's grip on the
             field, which swapping between two inputs would all drop. -->
        <input
          class="field"
          type={reveal ? "text" : "password"}
          value={password}
          oninput={(event) => {
            password = event.currentTarget.value;
            touched = true;
          }}
          autocomplete="current-password"
          disabled={app.connecting}
        />
        <button
          type="button"
          class="peek"
          onclick={() => (reveal = !reveal)}
          disabled={app.connecting}
          title={reveal ? "Hide password" : "Show password"}
          aria-label={reveal ? "Hide password" : "Show password"}
          aria-pressed={reveal}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            {#if reveal}
              <path
                d="M2.1 3.5 3.5 2.1l18.4 18.4-1.4 1.4-3.2-3.2A11 11 0 0 1 12 20c-5 0-9.3-3.1-11-8a12.7 12.7 0 0 1 4-5.4L2.1 3.5Zm4.3 4.3A10.7 10.7 0 0 0 3.2 12 10 10 0 0 0 12 18c1.3 0 2.6-.3 3.7-.8l-1.8-1.8a4 4 0 0 1-5.3-5.3L6.4 7.8ZM12 4c5 0 9.3 3.1 11 8a12.8 12.8 0 0 1-2.5 3.9l-1.4-1.4A10.8 10.8 0 0 0 20.8 12 10 10 0 0 0 12 6c-.5 0-1 0-1.5.1L8.9 4.5C9.9 4.2 10.9 4 12 4Z"
              />
            {:else}
              <path
                d="M12 4c5 0 9.3 3.1 11 8-1.7 4.9-6 8-11 8s-9.3-3.1-11-8c1.7-4.9 6-8 11-8Zm0 2a10 10 0 0 0-8.8 6 10 10 0 0 0 17.6 0A10 10 0 0 0 12 6Zm0 2a4 4 0 1 1 0 8 4 4 0 0 1 0-8Zm0 2a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z"
              />
            {/if}
          </svg>
        </button>
      </div>
    </label>

    <label class="check">
      <input type="checkbox" bind:checked={remember} disabled={app.connecting} />
      <span>Remember password</span>
    </label>

    {#if app.loginError}
      <p class="error" role="alert">{app.loginError}</p>
    {/if}
    {#if app.settingsError}
      <p class="error" role="alert">{app.settingsError}</p>
    {/if}

    <button class="btn primary" type="submit" disabled={!canSubmit}>
      {app.connecting ? "Connecting…" : app.addingAccount ? "Add account" : "Sign in"}
    </button>

    {#if app.addingAccount}
      <!-- The session is still running behind this; without a way back the
           only exit from adding an account was to sign into one. -->
      <button class="back" type="button" onclick={() => (app.addingAccount = false)}>
        <span aria-hidden="true">←</span> Back to {app.username}
      </button>
    {/if}
  </form>
  {/if}
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
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 13px;
    background: linear-gradient(140deg, var(--accent), var(--accent-hover));
    box-shadow: 0 6px 22px -6px var(--accent);
  }
  .mark svg {
    width: 26px;
    height: 26px;
    fill: #fff;
  }

  .back {
    justify-self: start;
    margin-top: 2px;
    padding: 5px 2px;
    font-size: 12.5px;
    color: var(--text-3);
    transition: color var(--fast);
  }
  .back:hover {
    color: var(--text-1);
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
  }  .check span {
    font-weight: 400;
    color: var(--text-2);
  }

  .waiting {
    display: grid;
    justify-items: center;
    gap: 14px;
    animation: rise var(--spring) both;
  }
  .waiting p {
    font-size: 13px;
    color: var(--text-2);
  }
  .waiting strong {
    color: var(--text-1);
    font-weight: 600;
  }

  .displaced {
    display: grid;
    gap: 7px;
    padding: 12px 13px;
    border-radius: var(--radius-sm);
    background: var(--danger-quiet);
    font-size: 12.5px;
    color: var(--danger);
  }
  .displaced .why {
    color: var(--text-2);
  }
  .displaced .btn {
    margin-top: 3px;
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

  /* The reveal control sits inside the field rather than beside it, so the
     row is still one input as far as the eye is concerned. */
  .secret {
    position: relative;
    display: flex;
  }
  .secret .field {
    flex: 1;
    padding-right: 36px;
  }
  .peek {
    position: absolute;
    top: 50%;
    right: 4px;
    transform: translateY(-50%);
    padding: 5px;
    border-radius: 6px;
    color: var(--text-3);
    transition: color var(--fast), background var(--fast);
  }
  .peek:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .peek:disabled {
    opacity: 0.5;
  }
  .peek svg {
    display: block;
    width: 15px;
    height: 15px;
    fill: currentColor;
  }
</style>
