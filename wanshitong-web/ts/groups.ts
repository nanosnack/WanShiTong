// Registers `groupSelection`, the Alpine component for selecting things that
// come in nested groups, e.g. the assertions page's assertions, grouped by the
// files they're about, within the changeset that made them. Each thing has a
// checkbox that belongs to the action bar's form, and each group a checkbox
// of its own, marked `data-selects`, which selects everything in the group.
//
// A group is the nearest `[data-selection-scope]` element around its
// checkbox, and holds everything, and every group, within it. A group's
// checkbox follows what's selected under it: checked while all of it is,
// partly checked while some of it is. Only the things are sent, and counted;
// a group's checkbox has no name.

interface GroupSelection {
  // How many things are selected. The action bar displays this count and
  // hides when it is zero.
  selected: number;
  count(): void;
  clear(): void;
  changed(event: Event): void;
  sync(): void;
}

interface GroupSelectionMagics {
  $el: HTMLElement;
  // The component's element, which $el isn't when a handler on an element
  // inside it calls a method, e.g. the action bar's Clear button.
  $root: HTMLElement;
}

// A group's checkbox.
const groupSelector = "input[type=checkbox][data-selects]";
// A thing's checkbox.
const thingSelector = "input[type=checkbox]:not([data-selects])";

// The things a group's checkbox selects.
function groupThings(group: HTMLInputElement): HTMLInputElement[] {
  const scope = group.closest("[data-selection-scope]");
  return scope ? Array.from(scope.querySelectorAll<HTMLInputElement>(thingSelector)) : [];
}

document.addEventListener("alpine:init", () => {
  Alpine.data("groupSelection", (): GroupSelection & ThisType<GroupSelection & GroupSelectionMagics> => ({
    selected: 0,

    count() {
      this.selected = this.$root.querySelectorAll(`${thingSelector}:checked`).length;
    },

    // Unselects everything, from the action bar's Clear button.
    clear() {
      for (const box of this.$root.querySelectorAll<HTMLInputElement>("input[type=checkbox]")) {
        box.checked = false;
        box.indeterminate = false;
      }
      this.selected = 0;
    },

    changed(event) {
      const box = event.target;
      if (!(box instanceof HTMLInputElement) || box.type !== "checkbox") {
        return;
      }
      if (box.matches(groupSelector)) {
        for (const thing of groupThings(box)) {
          thing.checked = box.checked;
        }
      }
      this.sync();
      this.count();
    },

    // Shows each group as selected, partly or not at all, from its things.
    sync() {
      for (const group of this.$root.querySelectorAll<HTMLInputElement>(groupSelector)) {
        const things = groupThings(group);
        const checked = things.filter((thing) => thing.checked).length;
        group.checked = things.length > 0 && checked === things.length;
        group.indeterminate = checked > 0 && checked < things.length;
      }
    },
  }));
});
