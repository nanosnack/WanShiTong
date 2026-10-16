//! Not from the topcoat registry: written for Wan Shi Tong in the same style.

use topcoat::{
    Result,
    view::{Attributes, StaticClass, View, class, component, view},
};

const TRACK: StaticClass =
    class!("relative h-2 w-full overflow-hidden rounded-full bg-foreground/10");

const BAR: StaticClass = class!("h-full rounded-full bg-primary transition-[width] duration-500");

/// A bar filled to `value` out of `max`. An empty `max` shows an empty bar.
///
/// Attributes go on the track, with classes added to its classes. Give it an
/// `aria-label` saying what's progressing.
///
/// ```ignore
/// view! {
///     progress(value: 40, max: 100, attrs: attributes! { aria-label="Files recorded" })
/// }
/// ```
#[component]
pub async fn progress(value: u64, max: u64, #[default] mut attrs: Attributes) -> Result<impl View> {
    let percent = if max == 0 {
        0.0
    } else {
        (value.min(max) as f64 / max as f64) * 100.0
    };
    Ok(view! {
        <div
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax=(max.to_string())
            aria-valuenow=(value.min(max).to_string())
            class=(class!(TRACK, attrs.remove("class")))
            (attrs)
        >
            <div class=(BAR) style=(format!("width: {percent:.1}%"))></div>
        </div>
    })
}
