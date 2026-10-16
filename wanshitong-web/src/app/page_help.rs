use crate::icons::{INFO, lucide};
use crate::ui::button::{ButtonSize, ButtonVariant, button};
use topcoat::{
    Result,
    view::{Attributes, Child, View, attributes, class, component, view},
};

/// An info button to put beside a page's heading, which opens a popover saying
/// what the page is for. `child` is the popover's paragraphs.
///
/// The button is a popover invoker, so it needs no script, and the popover
/// closes on Escape or a click outside it. Where the browser can anchor it, it
/// hangs below the button, across the screen on narrow ones; elsewhere it's
/// centered. A page has at most one.
///
/// `section` is for a page whose heading is `SECTION_H1`, hidden in the compact
/// header: there the button beside it is hidden too, and [`nav_page_help`]
/// shows one beside the header's page name instead.
#[component]
pub async fn page_help(#[default] section: bool, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        page_help_button(
            attrs: attributes! { class=(section.then_some("compact:hidden")) }
        )
        <div
            id="page-help"
            popover=""
            data-section-help=(section.then_some(""))
            class=(class!(
                "w-[min(65ch,calc(100vw-2rem))] space-y-2 rounded-lg border border-border",
                "bg-popover px-4 py-3 text-sm text-popover-foreground shadow-lg",
                "[position-anchor:--page-help] [position-area:bottom_span-right]",
                "[position-try-fallbacks:flip-inline] max-sm:[position-area:bottom_span-all]",
                "max-sm:justify-self-center supports-[position-area:bottom]:m-0",
                "supports-[position-area:bottom]:mt-2",
            ))
        >
            (child)
        </div>
    })
}

/// The header's button for a section page's help, shown in the compact header
/// when the page has one. Needs `group/page` on the `<body>`.
///
/// Both buttons are named as the popover's anchor; a hidden one has no box, so
/// the popover hangs below whichever is shown.
#[component]
pub async fn nav_page_help() -> Result<impl View> {
    Ok(view! {
        <div class="hidden compact:group-has-[[data-section-help]]/page:block">
            page_help_button()
        </div>
    })
}

#[component]
async fn page_help_button(#[default] mut attrs: Attributes) -> Result<impl View> {
    Ok(view! {
        button(
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Icon,
            attrs: attributes! {
                type="button"
                class=(class!(
                    "size-7 text-muted-foreground [anchor-name:--page-help]",
                    attrs.remove("class"),
                ))
                aria-label="About this page"
                title="About this page"
                popovertarget="page-help"
            },
            lucide(icon: INFO, attrs: attributes! { class="size-4" })
        )
    })
}
