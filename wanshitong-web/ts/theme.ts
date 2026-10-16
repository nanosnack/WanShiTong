// Applies the color theme: the one picked in the header's theme menu, or else
// the system's. The page loads this without `defer`, so it runs before the
// page is painted, which avoids a flash of the wrong theme.
//
// Also registers the theme menu's Alpine component, `themePicker`.

type Theme = "system" | "light" | "dark";

const storageKey = "theme";
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

function isTheme(value: string | null): value is Theme {
  return value === "system" || value === "light" || value === "dark";
}

// Storage can be unavailable (private browsing, blocked site data), in which
// case the system theme applies.
function pickedTheme(): Theme {
  try {
    const stored = localStorage.getItem(storageKey);
    return isTheme(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

function pickTheme(theme: Theme): void {
  try {
    if (theme === "system") {
      localStorage.removeItem(storageKey);
    } else {
      localStorage.setItem(storageKey, theme);
    }
  } catch {
    // Not remembered, but still applied to this page below.
  }
  applyTheme(theme);
}

function applyTheme(theme: Theme = pickedTheme()): void {
  const dark = theme === "dark" || (theme === "system" && systemDark.matches);
  document.documentElement.classList.toggle("dark", dark);
  // Native controls and scrollbars follow this.
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
  // The browser's toolbar, or an installed app's title bar, follows this.
  // The stylesheet loads before this script, so the variable is resolved.
  const background = getComputedStyle(document.documentElement).getPropertyValue("--background");
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", background.trim());
}

applyTheme();
systemDark.addEventListener("change", () => applyTheme());
// A theme picked in another tab.
window.addEventListener("storage", (event) => {
  if (event.key === storageKey) applyTheme();
});

interface ThemePicker {
  theme: Theme;
  pick(theme: Theme): void;
}

document.addEventListener("alpine:init", () => {
  Alpine.data("themePicker", (): ThemePicker & ThisType<ThemePicker & { $root: HTMLDetailsElement }> => ({
    theme: pickedTheme(),
    pick(theme) {
      pickTheme(theme);
      this.theme = theme;
      this.$root.open = false;
    },
  }));
});
