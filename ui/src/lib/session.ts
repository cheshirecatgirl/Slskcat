/**
 * Signing in, and moving between accounts.
 *
 * Four screens can start a session: the form, the launch, the switcher and a
 * proxy change. Each has to set the same flags in the same order and undo them
 * the same way on a refusal, so none of them does it itself.
 */

import { core } from "./core";
import { app } from "./state.svelte";
import type { Settings } from "./types";

/**
 * Sign in, and hold the flags that decide what is on screen while it happens.
 *
 * `waiting` replaces the form with a line for the length of the handshake,
 * which suits a sign-in nobody typed. Leave it null when someone is looking at
 * their own form, so a refusal lands back on the fields they filled in.
 */
export async function signIn(
  settings: Settings,
  waiting: "connecting" | "reconnecting" | null = null,
): Promise<Settings | null> {
  app.connecting = true;
  app.waiting = waiting;
  app.loginError = null;
  try {
    const saved = await core.connect(settings);
    app.settings = saved;
    return saved;
  } catch (error) {
    app.connecting = false;
    app.waiting = null;
    app.loginError = String(error);
    return null;
  }
}

/**
 * Make an account current and end the session.
 *
 * Switching does not sign in unless the password is remembered. Signing in on
 * someone's behalf with a stored password is how you end up connected as an
 * account you only meant to glance at, and the server treats a second login as
 * a takeover of the first.
 */
export async function switchTo(username: string) {
  // Read before the disconnect clears it. Leaving a name is the only thing
  // that can offer a way back to it.
  const leaving = app.connected ? app.username : app.previousAccount;
  app.previousAccount = leaving && leaving !== username ? leaving : null;

  try {
    const next = await core.switchAccount(username);
    app.settings = next;

    // The wait is held across the disconnect so the form never appears between
    // the two sessions: with a remembered password there is nothing to fill
    // in, and a form shown for the length of a handshake reads as a flicker.
    const remembered = next.password.length > 0;
    app.connecting = remembered;
    app.waiting = remembered ? "connecting" : null;
    app.loginError = null;

    await core.disconnect();
    if (remembered) await signIn(next, "connecting");
  } catch (error) {
    app.connecting = false;
    app.waiting = null;
    app.loginError = String(error);
  }
}
