//! Tailwind classes shared by pages. Tailwind finds them because they are
//! string literals in `src/`.

/// A page's heading.
pub const H1: &str = "text-2xl font-semibold tracking-tight";
/// The heading of a page the navigation links to. In the compact header (see
/// `compact` in styles.css) the header already names the page, so the heading
/// is only kept for screen readers.
pub const SECTION_H1: &str = "text-2xl font-semibold tracking-tight compact:sr-only";
/// The row holding a section page's heading and help button, when that's all it
/// holds. In the compact header both are hidden, so the row is dropped from the
/// layout rather than leaving a gap; the heading stays for screen readers.
pub const SECTION_HEADING_ROW: &str = "flex items-center gap-2 compact:contents";
/// A section heading.
pub const H2: &str = "text-lg font-semibold";
/// Identifiers, paths and timestamps.
pub const MONO: &str = "font-mono text-xs";
/// A `<details>` disclosure panel.
pub const DETAILS: &str = concat!(
    "rounded-lg border border-border px-4 py-3 text-sm ",
    "[&>summary]:cursor-pointer [&>summary]:font-medium [&>summary]:[overflow-wrap:anywhere] ",
    "[&[open]>summary]:mb-3",
);
/// An inline link. Touch screens have no hover, so it is always underlined there.
pub const LINK: &str = "font-medium underline-offset-4 hover:underline pointer-coarse:underline";
/// A `<table>` cell holding a number.
pub const NUMBER: &str = "text-right tabular-nums";
/// A `<dl>` of labelled facts, e.g. a changeset's author and date: labels down
/// the left, values beside them, on their baselines, as the values' sizes
/// differ.
pub const FACTS: &str =
    "grid grid-cols-[auto_minmax(0,1fr)] items-baseline gap-x-3 gap-y-0.5 text-sm";
/// A fact's label in [`FACTS`].
pub const FACT_LABEL: &str = "text-muted-foreground";
