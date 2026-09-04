/**
 * Moving between accounts.
 *
 * Two places need this: the switcher, which leaves an account, and the sign-in
 * form, which has to get back to the one it left when the account it moved to
 * turns out not to work. Keeping it in one place is what stops the way out and
 * the way back from drifting apart.
 */

import { core } from "./core";
import { app } from "./state.svelte";

/**
 * Make an account current and end the session.
 *
 * Switching deliberately does not sign in unless the password is remembered:
 * signing in on someone's behalf with a stored password is how you end up
 * connected as an account you only meant to glance at, and the server treats a
 * second login as a takeover of the first.
 */
export async function switchTo(username: string) {
  // Captured before the disconnect clears it. A switch away from a name is
  // the only thing that can offer a way back to it.
  const leaving = app.connected ? app.username : app.previousAccount;
  app.previousAccount = leaving && leaving !== username ? leaving : null;

  try {
    const next = await core.switchAccount(username);
    app.settings = next;
    // Held across the disconnect so the sign-in form never appears between
    // the two sessions: with a remembered password there is nothing to fill
    // in, and a form shown for the length of a handshake reads as a flicker.
    app.connecting = next.password.length > 0;
    app.resuming = app.connecting;
    app.loginError = null;
    await core.disconnect();
    if (next.password.length > 0) {
      app.settings = await core.connect(next);
    }
  } catch (error) {
    app.connecting = false;
    app.resuming = false;
    app.loginError = String(error);
  }
}
