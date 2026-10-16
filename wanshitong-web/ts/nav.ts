// Switches to the compact header, with the navbar's links in its menu, when
// they don't all fit in a row, by marking <body> `data-nav-collapsed` (see
// `compact` in styles.css). Narrow screens have the compact header anyway,
// without this. The page loads this right after the nav and without `defer`,
// so the header is already compact when it's first painted.

{
  const nav = document.getElementById("site-nav");
  if (nav) {
    const fit = (): void => {
      // Shown, the row overflows the nav if it doesn't fit. Measuring it
      // forces a layout but not a paint, so it's never seen overflowing.
      document.body.removeAttribute("data-nav-collapsed");
      document.body.toggleAttribute("data-nav-collapsed", nav.scrollWidth > nav.clientWidth);
    };
    fit();
    // The nav resizes with the window; its links, as fonts load.
    const observer = new ResizeObserver(fit);
    observer.observe(nav);
    const row = nav.querySelector("[data-nav-row]");
    if (row) {
      observer.observe(row);
    }
  }
}
