//! Lucide icons (<https://lucide.dev>, ISC), inline.
//!
//! Each icon is its SVG contents, copied from `lucide-static`. They're copied
//! rather than fetched: topcoat's `iconify_icon!` downloads its icon set during
//! the build. To add one, copy the children of its `<svg>` from
//! `https://cdn.jsdelivr.net/npm/lucide-static/icons/<name>.svg`.

use topcoat::{
    Result,
    view::{Attributes, Unescaped, View, component, view},
};

/// An icon's SVG contents. Only constructed here, from the constants below, so
/// safe to render unescaped.
#[derive(Clone, Copy)]
pub struct Icon(&'static str);

pub const ARROW_UP_RIGHT: Icon =
    Icon(r#"<path d="M7 7h10v10"></path><path d="M7 17 17 7"></path>"#);
pub const CHECK: Icon = Icon(r#"<path d="M20 6 9 17l-5-5"></path>"#);
pub const CHEVRON_RIGHT: Icon = Icon(r#"<path d="m9 18 6-6-6-6"></path>"#);
pub const COPY: Icon = Icon(concat!(
    r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect>"#,
    r#"<path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>"#,
));
pub const DISC: Icon =
    Icon(r#"<circle cx="12" cy="12" r="10"></circle><circle cx="12" cy="12" r="2"></circle>"#);
pub const EYE: Icon = Icon(concat!(
    r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"></path>"#,
    r#"<circle cx="12" cy="12" r="3"></circle>"#,
));
pub const EYE_OFF: Icon = Icon(concat!(
    r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"></path>"#,
    r#"<path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"></path>"#,
    r#"<path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"></path>"#,
    r#"<path d="m2 2 20 20"></path>"#,
));
pub const FOLDER: Icon = Icon(
    r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"></path>"#,
);
pub const IMAGE: Icon = Icon(concat!(
    r#"<rect width="18" height="18" x="3" y="3" rx="2" ry="2"></rect>"#,
    r#"<circle cx="9" cy="9" r="2"></circle>"#,
    r#"<path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"></path>"#,
));
pub const INFO: Icon = Icon(concat!(
    r#"<circle cx="12" cy="12" r="10"></circle>"#,
    r#"<path d="M12 16v-4"></path>"#,
    r#"<path d="M12 8h.01"></path>"#,
));
pub const MENU: Icon =
    Icon(r#"<path d="M4 5h16"></path><path d="M4 12h16"></path><path d="M4 19h16"></path>"#);
pub const MINUS: Icon = Icon(r#"<path d="M5 12h14"></path>"#);
pub const MONITOR: Icon = Icon(concat!(
    r#"<rect width="20" height="14" x="2" y="3" rx="2"></rect>"#,
    r#"<path d="M8 21h8"></path>"#,
    r#"<path d="M12 17v4"></path>"#,
));
pub const MOON: Icon = Icon(r#"<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"></path>"#);
pub const PENCIL: Icon = Icon(concat!(
    r#"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"></path>"#,
    r#"<path d="m15 5 4 4"></path>"#,
));
pub const SUN: Icon = Icon(concat!(
    r#"<circle cx="12" cy="12" r="4"></circle>"#,
    r#"<path d="M12 2v2"></path>"#,
    r#"<path d="M12 20v2"></path>"#,
    r#"<path d="m4.93 4.93 1.41 1.41"></path>"#,
    r#"<path d="m17.66 17.66 1.41 1.41"></path>"#,
    r#"<path d="M2 12h2"></path>"#,
    r#"<path d="M20 12h2"></path>"#,
    r#"<path d="m6.34 17.66-1.41 1.41"></path>"#,
    r#"<path d="m19.07 4.93-1.41 1.41"></path>"#,
));
pub const X: Icon = Icon(r#"<path d="M18 6 6 18"></path><path d="m6 6 12 12"></path>"#);

/// A Lucide icon, inline. `attrs` go on the `<svg>`.
#[component]
pub async fn lucide(icon: Icon, #[default] attrs: Attributes) -> Result<impl View> {
    Ok(view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            (attrs)
        >
            (Unescaped::new_unchecked(icon.0))
        </svg>
    })
}
