// Registers `selection`, the Alpine component for selecting posters in the
// catalog's grid, to mark them as needing correction (see `poster_grid` in
// catalog.rs). Each poster has a checkbox, drawn as a circle in its corner.
//
// With a mouse, a poster's circle shows while it's hovered, and every circle
// shows once any is selected. On a touch screen there's no hover, so the
// circles stay hidden until a long press on a poster selects it. Either way,
// from then until nothing is selected, a click or tap anywhere on a poster
// selects or unselects it rather than opening it.

interface PosterSelection {
  // How many files are selected. The mark bar displays this count and hides
  // when it is zero.
  selected: number;
  count(): void;
  clear(): void;
  more(row: HTMLElement): void;
  pointerDown(event: PointerEvent): void;
  pointerEnd(): void;
  click(event: MouseEvent): void;
  contextMenu(event: Event): void;
}

interface PosterSelectionMagics {
  $el: HTMLElement;
  // The component's element, which $el isn't when a handler on an element
  // inside it calls a method, e.g. the mark bar's Clear button.
  $root: HTMLElement;
  $ajax(url: string, options: { target: string }): Promise<void>;
}

// The checkboxes for the mark bar; see `mark_selected_bar` in corrections.rs.
const selectableSelector = "input[form=mark-selected]";
// How long a press selects, rather than taps, in milliseconds.
const longPressDelay = 500;
// How far a touch can move, in pixels, and still be a press, not a scroll.
const longPressSlop = 10;

// The checkbox of the poster an event happened in, if any.
function posterCheckbox(target: EventTarget | null): HTMLInputElement | null {
  if (!(target instanceof Element)) {
    return null;
  }
  return target.closest("li")?.querySelector<HTMLInputElement>(selectableSelector) ?? null;
}

document.addEventListener("alpine:init", () => {
  // The press in progress, kept outside Alpine's reactive state.
  let pressTimer: number | undefined;
  let pressStart: { x: number; y: number } | undefined;
  // Set when a long press selected a poster, so that the click that follows
  // releasing it doesn't open the poster too.
  let pressed = false;
  // Whether the last pointer was a touch, as only touches select by a long
  // press.
  let touched = false;

  Alpine.data("selection", (): PosterSelection & ThisType<PosterSelection & PosterSelectionMagics> => ({
    selected: 0,

    count() {
      this.selected = this.$el.querySelectorAll(`${selectableSelector}:checked`).length;
    },

    // Unselects everything, from the mark bar's Clear button.
    clear() {
      for (const checkbox of this.$root.querySelectorAll<HTMLInputElement>(selectableSelector)) {
        checkbox.checked = false;
      }
      this.selected = 0;
    },

    // Loads the next page of posters once the "Loading more…" row after the
    // loaded ones scrolls into view.
    more(row) {
      const { next, target } = row.dataset;
      if (!next || !target) {
        return;
      }
      this.$ajax(next, { target }).then(() => row.remove());
    },

    pointerDown(event) {
      touched = event.pointerType === "touch";
      if (!touched || this.selected > 0) {
        return;
      }
      const checkbox = posterCheckbox(event.target);
      if (!checkbox) {
        return;
      }
      pressStart = { x: event.clientX, y: event.clientY };
      const onMove = (move: PointerEvent) => {
        if (
          pressStart &&
          Math.hypot(move.clientX - pressStart.x, move.clientY - pressStart.y) > longPressSlop
        ) {
          this.pointerEnd();
        }
      };
      document.addEventListener("pointermove", onMove, { once: false });
      pressTimer = window.setTimeout(() => {
        document.removeEventListener("pointermove", onMove);
        pressTimer = undefined;
        pressed = true;
        checkbox.checked = true;
        this.count();
      }, longPressDelay);
      const end = () => {
        document.removeEventListener("pointermove", onMove);
        this.pointerEnd();
      };
      document.addEventListener("pointerup", end, { once: true });
      document.addEventListener("pointercancel", end, { once: true });
    },

    pointerEnd() {
      window.clearTimeout(pressTimer);
      pressTimer = undefined;
      pressStart = undefined;
    },

    click(event) {
      if (pressed) {
        // The release of a long press, which selected the poster already.
        pressed = false;
        event.preventDefault();
        return;
      }
      // A click that opens the poster elsewhere, e.g. in a new tab, still does.
      if (this.selected === 0 || event.ctrlKey || event.metaKey || event.shiftKey) {
        return;
      }
      const checkbox = posterCheckbox(event.target);
      if (!checkbox || event.target === checkbox) {
        return;
      }
      // A click or tap while selecting toggles the poster rather than opening it.
      event.preventDefault();
      checkbox.checked = !checkbox.checked;
      this.count();
    },

    contextMenu(event) {
      // A long press on a link would open the browser's menu for it.
      if (touched && posterCheckbox(event.target)) {
        event.preventDefault();
      }
    },
  }));
});
