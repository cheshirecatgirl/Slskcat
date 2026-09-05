/**
 * Close a menu when attention leaves it.
 *
 * A pointer down anywhere outside and the Escape key mean the same thing, and
 * every menu in the app owes the same answer to both.
 *
 * Apply it to the element that holds the menu *and* its trigger. If it were on
 * the menu alone, clicking the trigger to close would count as outside, and
 * the trigger would immediately reopen what this had just shut.
 */
import type { Action } from "svelte/action";

export const dismiss: Action<HTMLElement, () => void> = (node, onclose) => {
  let close = onclose;

  const away = (event: PointerEvent) => {
    if (!node.contains(event.target as Node)) close();
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key === "Escape") close();
  };

  // `pointerdown` in the capture phase: the menu is gone before whatever was
  // pressed reacts, and a handler that stops propagation cannot keep it open.
  document.addEventListener("pointerdown", away, true);
  document.addEventListener("keydown", escape);

  return {
    update: (next: () => void) => (close = next),
    destroy: () => {
      document.removeEventListener("pointerdown", away, true);
      document.removeEventListener("keydown", escape);
    },
  };
};
