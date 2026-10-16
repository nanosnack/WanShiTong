// Registers `posterSize`, the Alpine component for the catalog grid's poster
// size slider. It remembers the size in this browser.
//
// The size is how many posters fit in a row, since the grid stretches them to
// fill it: a width between two column counts would look the same as one of
// them. The slider only offers as many as fit in the row, which the grid would
// otherwise quietly cap (see `poster-grid` in styles.css). Its value is the
// negated count, so that dragging it right makes the posters bigger, and the
// value stays put when the range changes with the row's width.
//
// Pages of posters don't come in whole rows, so while another page is coming,
// the posters past the last full row are held back, and the grid doesn't end
// ragged until the next page fills it in. The last page shows them all.

interface PosterSize {
  // The chosen count, which a narrow row can show fewer of.
  columns: number;
  // The most that fit in the row, measured.
  fitting: number;
  readonly shown: number;
  // The slider's value.
  size: number;
  init(): void;
}

interface Magics {
  $el: HTMLElement;
  $watch(name: string, callback: (value: number) => void): void;
}

const posterColumnsKey = "poster-columns";
// The slider's range and the grid's default, as in catalog.rs and styles.css.
const minPosterColumns = 2;
const maxPosterColumns = 10;
const defaultPosterColumns = 4;
// The grid's narrowest poster and its column gap, in rem, as in styles.css.
const minPosterWidth = 3;
const posterGap = 1;

// Storage can be unavailable (private browsing, blocked site data), in which
// case the default applies.
function storedPosterColumns(): number {
  try {
    const stored = Number(localStorage.getItem(posterColumnsKey));
    return Number.isInteger(stored) && stored >= minPosterColumns && stored <= maxPosterColumns
      ? stored
      : defaultPosterColumns;
  } catch {
    return defaultPosterColumns;
  }
}

// How many posters fit in a row this wide, as the grid lays them out.
function fittingPosterColumns(width: number): number {
  const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
  const fitting = Math.floor((width + posterGap * rem) / ((minPosterWidth + posterGap) * rem));
  return Math.min(maxPosterColumns, Math.max(minPosterColumns, fitting));
}

// Hides the posters in `grid` past its last full row of `columns`, while it
// has the item that loads the next page (`data-next`, see catalog.rs).
function holdBackPartialRow(grid: HTMLElement, columns: number): void {
  const items = Array.from(grid.querySelectorAll<HTMLElement>("#catalog-results > li"));
  const posters = items.filter((item) => item.dataset.next === undefined);
  const held = posters.length === items.length ? 0 : posters.length % columns;
  posters.forEach((poster, i) => {
    poster.hidden = i >= posters.length - held;
  });
}

document.addEventListener("alpine:init", () => {
  Alpine.data("posterSize", (): PosterSize & ThisType<PosterSize & Magics> => ({
    columns: storedPosterColumns(),
    fitting: maxPosterColumns,
    get shown() {
      return Math.min(this.columns, this.fitting);
    },
    get size() {
      return -this.shown;
    },
    set size(size) {
      this.columns = -size;
    },
    init() {
      // The component spans the grid's width.
      new ResizeObserver(([entry]) => {
        this.fitting = fittingPosterColumns(entry.contentRect.width);
      }).observe(this.$el);
      // Pages are appended and searches replace the grid, both within this.
      const holdBack = () => holdBackPartialRow(this.$el, this.shown);
      new MutationObserver(holdBack).observe(this.$el, { childList: true, subtree: true });
      this.$watch("shown", holdBack);
      holdBack();
      this.$watch("columns", (columns) => {
        try {
          localStorage.setItem(posterColumnsKey, String(columns));
        } catch {
          // Not remembered, but still applied to this page.
        }
      });
    },
  }));
});
