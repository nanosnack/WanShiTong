// Registers `fileTree`, the Alpine component for selecting files and folders
// in the files page's tree, or its search results, to mark them as needing
// correction (see `page` in files.rs). Each file and folder has a checkbox for
// the mark bar's form.
//
// Checking a folder selects everything under it, including what hasn't loaded
// yet: the folder is marked as a whole, and the files under it that load later
// arrive checked. Unchecking anything under a checked folder leaves the folder
// partly selected, and what's still checked under it is marked instead. Only
// the topmost selections are sent, and counted.
//
// The tree nests each folder's rows in its `<details>`. Search results are a
// flat list instead, each row with its `data-depth`: a folder's rows are those
// after it that are deeper, up to the next that isn't.

interface FileTree {
  // How many files are selected. The mark bar displays this count and hides
  // when it is zero.
  selected: number;
  count(): void;
  clear(): void;
  changed(event: Event): void;
  inherit(folder: HTMLElement): void;
  inheritRows(list: HTMLElement | null): void;
  load(folder: HTMLDetailsElement): void;
  more(row: HTMLElement): void;
  openSingle(list: Element | null): void;
  selectAll(event: Event): void;
  submitting(): void;
}

interface FileTreeMagics {
  $el: HTMLElement;
  // The component's element, which $el isn't when a handler on an element
  // inside it calls a method, e.g. Select all's checkbox.
  $root: HTMLElement;
  $ajax(url: string, options: { target: string }): Promise<void>;
}

// The checkboxes for the mark bar; see `mark_selected_bar` in corrections.rs.
const treeSelectableSelector = "input[form=mark-selected]";

// A row's depth in a flat list of search results, if it's in one.
function rowDepth(row: Element | null): number | null {
  const depth = row instanceof HTMLElement ? row.dataset.depth : undefined;
  return depth === undefined ? null : Number(depth);
}

// A row's checkbox.
function rowBox(row: Element): HTMLInputElement | null {
  return row.querySelector<HTMLInputElement>("input[type=checkbox]");
}

// Selects or unselects what's loaded under a folder whose checkbox changed,
// and shows the folders that `box` is in as selected, partly or not at all.
function cascade(box: HTMLInputElement): void {
  if (box.name === "folder") {
    for (const descendant of descendantBoxes(box)) {
      descendant.checked = box.checked;
      descendant.indeterminate = false;
    }
  }
  // A folder stays checked only while everything loaded under it is, and is
  // partly selected while only some of it is.
  for (let folder = parentFolderBox(box); folder; folder = parentFolderBox(folder)) {
    const descendants = descendantBoxes(folder);
    if (folder.checked && descendants.some((descendant) => !descendant.checked)) {
      folder.checked = false;
    }
    folder.indeterminate =
      !folder.checked && descendants.some((descendant) => descendant.checked || descendant.indeterminate);
  }
}

// Cascades a change in a flat list of rows outside the files page's tree,
// e.g. a mark's files on the Corrections page, whose folder checkboxes select
// its files rather than being sent themselves.
function cascadeRows(event: Event): void {
  const box = event.target;
  if (box instanceof HTMLInputElement && rowDepth(box.closest("li")) !== null) {
    cascade(box);
  }
}

// Selects or unselects every row in such a list from a checkbox outside it,
// e.g. "Select all of its files" above a mark's.
function selectRows(all: HTMLInputElement, list: ParentNode): void {
  for (const box of list.querySelectorAll<HTMLInputElement>("li input[type=checkbox]")) {
    box.checked = all.checked;
    box.indeterminate = false;
  }
}

// The checkbox of the folder `box` is in, if it's in one.
function parentFolderBox(box: HTMLInputElement): HTMLInputElement | null {
  const row = box.closest("li");
  const depth = rowDepth(row);
  if (row && depth !== null) {
    for (let before = row.previousElementSibling; before; before = before.previousElementSibling) {
      const beforeDepth = rowDepth(before);
      if (beforeDepth !== null && beforeDepth < depth) {
        return before.querySelector<HTMLInputElement>("input[name=folder]");
      }
    }
    return null;
  }
  const folder = row?.parentElement?.closest("details");
  return folder?.querySelector<HTMLInputElement>(":scope > summary input[name=folder]") ?? null;
}

// The checkboxes of everything loaded under a folder.
function descendantBoxes(folderBox: HTMLInputElement): HTMLInputElement[] {
  const row = folderBox.closest("li");
  const depth = rowDepth(row);
  if (row && depth !== null) {
    const boxes = [];
    for (let after = row.nextElementSibling; after; after = after.nextElementSibling) {
      const afterDepth = rowDepth(after);
      if (afterDepth === null || afterDepth <= depth) {
        break;
      }
      const box = rowBox(after);
      if (box) {
        boxes.push(box);
      }
    }
    return boxes;
  }
  const folder = folderBox.closest("details");
  if (!folder) {
    return [];
  }
  return Array.from(folder.querySelectorAll<HTMLInputElement>(`:scope > ul ${treeSelectableSelector}`));
}

// Whether `box` is checked, and no folder it's in is: a selection of its own.
function isTopmost(box: HTMLInputElement): boolean {
  if (!box.checked) {
    return false;
  }
  for (let folder = parentFolderBox(box); folder; folder = parentFolderBox(folder)) {
    if (folder.checked) {
      return false;
    }
  }
  return true;
}

document.addEventListener("alpine:init", () => {
  Alpine.data("fileTree", (): FileTree & ThisType<FileTree & FileTreeMagics> => ({
    selected: 0,

    count() {
      const boxes = this.$root.querySelectorAll<HTMLInputElement>(`${treeSelectableSelector}:checked`);
      this.selected = Array.from(boxes).filter(isTopmost).length;
    },

    // Unselects everything, from the mark bar's Clear button.
    clear() {
      for (const box of this.$root.querySelectorAll<HTMLInputElement>(treeSelectableSelector)) {
        box.checked = false;
        box.indeterminate = false;
      }
      this.selected = 0;
    },

    changed(event) {
      const box = event.target;
      if (!(box instanceof HTMLInputElement) || !box.matches(treeSelectableSelector)) {
        return;
      }
      cascade(box);
      this.count();
    },

    // Loads a folder's children the first time it opens. If they're one
    // folder, that opens too, down to where there's more than one thing.
    load(folder) {
      if (folder.dataset.loaded) {
        return;
      }
      folder.dataset.loaded = "true";
      const { children, target } = folder.dataset;
      if (!children || !target) {
        return;
      }
      this.$ajax(children, { target }).then(() => {
        this.inherit(folder);
        this.openSingle(folder.querySelector(":scope > ul"));
      });
    },

    // Loads the next page of a folder's children, or of the search results,
    // once the "Loading more…" row after the loaded ones scrolls into view.
    // What arrives is checked if it's under a checked folder.
    more(row) {
      const { next, target } = row.dataset;
      if (!next || !target) {
        return;
      }
      this.$ajax(next, { target }).then(() => {
        const folder = row.closest("details");
        if (folder) {
          this.inherit(folder);
        } else {
          this.inheritRows(row.parentElement);
        }
        row.remove();
      });
    },

    // Selects or unselects everything in the tree, e.g. a work's files, from a
    // checkbox outside it.
    selectAll(event) {
      const all = event.target;
      const tree = this.$root.querySelector("#file-tree");
      if (!(all instanceof HTMLInputElement) || !tree) {
        return;
      }
      for (const row of tree.children) {
        const box = row.querySelector<HTMLInputElement>(treeSelectableSelector);
        if (box) {
          box.checked = all.checked;
          box.indeterminate = false;
          cascade(box);
        }
      }
      this.count();
    },

    // Opens the folder a list holds, if that's all it holds.
    openSingle(list) {
      const rows = Array.from(list?.children ?? []);
      const folder = rows.length === 1 ? rows[0].querySelector(":scope > details") : null;
      if (folder instanceof HTMLDetailsElement) {
        folder.open = true;
      }
    },

    // Checks what just loaded under a folder, if the folder is checked.
    inherit(folder) {
      const box = folder.querySelector<HTMLInputElement>(":scope > summary input[name=folder]");
      if (!box?.checked) {
        return;
      }
      for (const descendant of descendantBoxes(box)) {
        descendant.checked = true;
      }
    },

    // Checks the rows of a flat list that just loaded under a checked folder.
    inheritRows(list) {
      if (!list) {
        return;
      }
      // The folders open at each row, and whether each is checked.
      const open: { depth: number; checked: boolean }[] = [];
      for (const row of list.children) {
        const depth = rowDepth(row);
        const box = rowBox(row);
        if (depth === null || !box) {
          continue;
        }
        while (open.length > 0 && open[open.length - 1].depth >= depth) {
          open.pop();
        }
        if (open.some((folder) => folder.checked)) {
          box.checked = true;
        }
        if (box.name === "folder") {
          open.push({ depth, checked: box.checked });
        }
      }
    },

    // Sends only the topmost selections: a checked folder stands for
    // everything under it.
    submitting() {
      for (const box of this.$root.querySelectorAll<HTMLInputElement>(`${treeSelectableSelector}:checked`)) {
        if (!isTopmost(box)) {
          box.disabled = true;
        }
      }
    },
  }));
});
