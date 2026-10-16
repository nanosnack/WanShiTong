use topcoat::{
    Result,
    view::{Attributes, StaticClass, View, class, component, view},
};

/// Classes for the native checkbox input and its checked state.
const CHECKBOX: StaticClass = class!(
    "peer size-4 shrink-0 appearance-none rounded-[4px] border border-border \
     bg-background transition-colors outline-none \
     checked:border-primary checked:bg-primary \
     focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 \
     focus-visible:ring-offset-background disabled:pointer-events-none",
);

/// A styled native checkbox.
///
/// Pass input attributes and event handlers through `attrs`. Classes apply to the
/// wrapper, while other attributes go on the `<input>`. Use `checked` for the initial
/// state. The indeterminate state requires setting a DOM property and has no custom
/// styling.
///
/// ```ignore
/// view! {
///     <div class="flex items-center gap-2">
///         checkbox(attrs: attributes! { id="terms" name="terms" checked="" })
///         label(attrs: attributes! { for="terms" }, "Accept terms")
///     </div>
/// }
/// ```
#[component]
pub async fn checkbox(#[default] mut attrs: Attributes) -> Result<impl View> {
    // The checkmark cannot be drawn by the `<input>` itself, which renders no
    // children or pseudo-elements: it is a sibling icon overlaid on the
    // control, revealed by the input's `peer` state while checked.
    Ok(view! {
        <span
            class=(class!(
                "peer relative inline-flex shrink-0 has-[:disabled]:opacity-50",
                attrs.remove("class"),
            ))
        >
            <input type="checkbox" class=(CHECKBOX) (attrs)>
            // Lucide's "check", inline: the registry version uses an Iconify icon,
            // which downloads its icon set during the build.
            <svg
                class=(class!(
                    "pointer-events-none absolute inset-0 m-auto size-3.5 text-primary-foreground",
                    "opacity-0 peer-checked:opacity-100",
                ))
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <path d="M20 6 9 17l-5-5"></path>
            </svg>
        </span>
    })
}
