//! The first-run guide: a welcome on the home page, then a card at the bottom
//! of every page explaining the next step, which the user marks done when they
//! think it is. The bubble can be tucked away, leaving the owl, which brings it
//! back. It doesn't point at or lock any of the UI; the Config page turns it
//! off and on. Both are the owl talking.

use super::{catalog, corrections, data_sources, ingests, views};
use crate::icons::{MINUS, lucide};
use crate::{
    context::app,
    styles::LINK,
    ui::{
        button::{ButtonSize, ButtonVariant, button},
        card::{card, card_description, card_footer, card_header, card_title},
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    cookie::{Cookie, Cookies, SameSite, cookies, time},
    router::{
        content::Form,
        error::{SeeOther, see_other},
        href,
        request::uri,
        route,
    },
    runtime::{Event, Signal, procedure, signal},
    view::{Child, View, attributes, class, component, view},
};
use wanshitong_app::onboarding::{self, OnboardingStep};

/// Set by [`set_collapsed`] while the card is tucked away, so pages are rendered
/// that way.
const COLLAPSED_COOKIE: &str = "onboarding-collapsed";
/// How long the cookie lasts: 400 days, the most browsers keep one.
const COLLAPSED_COOKIE_MAX_AGE: time::Duration = time::Duration::days(400);

/// Sent by the card's buttons: the page to go back to, and for Done, the step
/// that's done.
#[derive(Deserialize)]
struct StepForm {
    step: Option<u32>,
    back_to: String,
}

impl StepForm {
    /// `back_to`, if it's a path on this site, else the catalog. A form can be
    /// posted from anywhere, so it mustn't redirect off the site.
    fn back(&self, cx: &Cx) -> String {
        let local = self.back_to.starts_with('/')
            && !self.back_to.starts_with("//")
            && !self.back_to.starts_with("/\\");
        if local {
            self.back_to.clone()
        } else {
            href!(catalog::page).resolve(cx)
        }
    }
}

/// Marks a step done and goes back to the page it was done from.
#[route(POST "./done")]
async fn done(cx: &Cx, Form(form): Form<StepForm>) -> Result<SeeOther> {
    if let Some(step) = form.step.and_then(OnboardingStep::from_index) {
        onboarding::complete_onboarding_step(app(cx), step).await?;
    }
    Ok(see_other(form.back(cx)))
}

/// Goes back to the step before, and to the page it was left from. Never back
/// to the welcome, which would hide the card.
#[route(POST "./previous")]
async fn previous(cx: &Cx, Form(form): Form<StepForm>) -> Result<SeeOther> {
    let step = form.step.and_then(OnboardingStep::from_index);
    if let Some(step) = step.filter(|step| step.number().is_some()) {
        onboarding::return_to_onboarding_step(app(cx), step).await?;
    }
    Ok(see_other(form.back(cx)))
}

/// Turns the guide off (the welcome's Skip) and goes to `back_to`.
#[route(POST "./hide")]
async fn hide(cx: &Cx, Form(form): Form<StepForm>) -> Result<SeeOther> {
    onboarding::set_onboarding_enabled(app(cx), false).await?;
    Ok(see_other(form.back(cx)))
}

/// Remembers whether the card is tucked away, from its buttons, so pages are
/// rendered that way.
#[procedure]
async fn set_collapsed(cx: &Cx, collapsed: bool) -> Result<()> {
    cookies(cx).add(
        Cookie::build((COLLAPSED_COOKIE, collapsed.to_string()))
            .path("/")
            .max_age(COLLAPSED_COOKIE_MAX_AGE)
            .same_site(SameSite::Lax),
    );
    Ok(())
}

/// The guide's card, if it's on a numbered step, in an element that's always
/// there, so its buttons can swap it for the card as it is after. The welcome
/// is the home page's instead (see [`welcome`]).
#[component]
pub async fn onboarding_card(cx: &Cx) -> Result<impl View> {
    let step = onboarding::get_onboarding(app(cx))
        .await?
        .current_step()
        .and_then(|step| Some((step, step.number()?)));
    // The step Previous goes back to, if it's a numbered one.
    let previous_step = step
        .and_then(|(step, _)| OnboardingStep::from_index(step.index().checked_sub(1)?))
        .filter(|step| step.number().is_some());
    let back_to = uri(cx)
        .path_and_query()
        .map_or("/", |path| path.as_str())
        .to_string();
    // Whether the bubble is showing, rather than tucked away, as the browser
    // last told the server.
    let collapsed = cookies(cx)
        .get(COLLAPSED_COOKIE)
        .is_some_and(|cookie| cookie.value() == "true");
    let open = signal(cx, || !collapsed);

    Ok(view! {
        <div id="onboarding">
            if let Some((step, number)) = step {
                // Tucked away, only the owl shows, in the corner, and the strip
                // it's in doesn't take clicks.
                <aside
                    aria-label="Getting started"
                    class=(class!(
                        "group pointer-events-none fixed inset-x-4 bottom-4 z-20",
                        "sm:left-auto sm:w-[28rem]",
                    ))
                    :data-open=$(if open.get() { "true" } else { "false" })
                >
                    owl_says(
                        owl_class: "size-14 group-data-[open=false]:size-11",
                        open: Some(open.clone()),
                        card(
                            attrs: attributes! { class="gap-3! py-4! shadow-lg!" },
                            card_header(
                                attrs: attributes! { class="px-4!" },
                                <div class="flex items-center justify-between gap-2">
                                    <p class="text-xs text-muted-foreground">
                                        (format!(
                                            "Getting started · Step {number} of {}",
                                            OnboardingStep::NUMBERED,
                                        ))
                                    </p>
                                    button(
                                        variant: ButtonVariant::Ghost,
                                        size: ButtonSize::Icon,
                                        attrs: attributes! {
                                            type="button"
                                            class="-my-2 -mr-2 size-7"
                                            aria-label="Tuck the guide away"
                                            title="Tuck the guide away. Click the owl to bring it back."
                                            @click=$(async |_e: Event| {
                                                open.toggle();
                                                set_collapsed(!open.get()).await;
                                            })
                                        },
                                        // "Minus", for tucking the bubble away: minimizing, not
                                        // closing.
                                        lucide(icon: MINUS, attrs: attributes! { class="size-4" })
                                    )
                                </div>
                                step_text(step: step)
                            )
                            card_footer(
                                attrs: attributes! { class="justify-end px-4!" },
                                if let Some(previous_step) = previous_step {
                                    <form
                                        method="post"
                                        action=(href!(previous))
                                        x-data=""
                                        x-target="onboarding"
                                    >
                                        <input
                                            type="hidden"
                                            name="step"
                                            value=(previous_step.index().to_string())
                                        >
                                        <input
                                            type="hidden"
                                            name="back_to"
                                            value=(back_to.as_str())
                                        >
                                        button(
                                            variant: ButtonVariant::Ghost,
                                            size: ButtonSize::Sm,
                                            attrs: attributes! { type="submit" },
                                            "Previous"
                                        )
                                    </form>
                                }
                                <form
                                    method="post"
                                    action=(href!(done))
                                    x-data=""
                                    x-target="onboarding"
                                >
                                    <input
                                        type="hidden"
                                        name="step"
                                        value=(step.index().to_string())
                                    >
                                    <input
                                        type="hidden"
                                        name="back_to"
                                        value=(back_to.as_str())
                                    >
                                    button(
                                        size: ButtonSize::Sm,
                                        attrs: attributes! { type="submit" },
                                        if step.is_last() {
                                            "Finish"
                                        } else {
                                            "Done"
                                        }
                                    )
                                </form>
                            )
                        )
                    )
                </aside>
            }
        </div>
    })
}

/// The first thing a new user sees: the owl, offering to show them around.
/// Starting goes to where the first step is done; skipping turns the guide off.
#[component]
pub async fn welcome(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <div class="flex min-h-[60vh] items-center justify-center">
            owl_says(
                owl_class: "size-24 sm:size-32",
                card(
                    attrs: attributes! { class="max-w-md shadow-lg!" },
                    card_header(
                        card_title(
                            attrs: attributes! { class="text-xl!" },
                            "Welcome to Wan Shi Tong"
                        )
                        card_description(
                            attrs: attributes! { class="mt-1" },
                            "I catalog your media, then lay it out in views your media "
                            "server can read, without ever changing your files."
                        )
                        card_description(
                            "I can show you around, one step at a time. First, tell me where "
                            "your media is."
                        )
                    )
                    card_footer(
                        attrs: attributes! { class="flex-wrap justify-end" },
                        <form method="post" action=(href!(hide))>
                            <input
                                type="hidden"
                                name="back_to"
                                value=(href!(catalog::page).resolve(cx))
                            >
                            button(
                                variant: ButtonVariant::Ghost,
                                attrs: attributes! {
                                    type="submit"
                                    title="It can be turned on again from the Config page."
                                },
                                "Skip the guide"
                            )
                        </form>
                        <form method="post" action=(href!(done))>
                            <input
                                type="hidden"
                                name="step"
                                value=(OnboardingStep::Welcome.index().to_string())
                            >
                            <input
                                type="hidden"
                                name="back_to"
                                value=(href!(data_sources::page).resolve(cx))
                            >
                            button(
                                attrs: attributes! { type="submit" },
                                "Add a data source"
                            )
                        </form>
                    )
                )
            )
        </div>
    })
}

/// The owl, with `child` (a card) beside it as a speech bubble. With `open`,
/// the bubble can be tucked away, and `open` says whether it's showing: the owl
/// is then a button that toggles it, and tells the server (see
/// [`set_collapsed`]).
#[component]
async fn owl_says(
    owl_class: &'static str,
    #[default] open: Option<Signal<bool>>,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let owl_class = format!("owl-avatar {owl_class} shrink-0");

    Ok(view! {
        <div class="flex items-center justify-end gap-3">
            if let Some(open) = &open {
                <button
                    type="button"
                    class=(format!(
                        "{owl_class} pointer-events-auto transition-[width,height]",
                    ))
                    aria-label="Getting started guide"
                    aria-controls="onboarding-bubble"
                    :aria-expanded=$(if open.get() { "true" } else { "false" })
                    @click=$(async |_e: Event| {
                        open.toggle();
                        set_collapsed(!open.get()).await;
                    })
                >
                    owl()
                </button>
            } else {
                <div class=(owl_class)>owl()</div>
            }
            <div
                id=(open.is_some().then_some("onboarding-bubble"))
                class="speech-bubble pointer-events-auto min-w-0 flex-1"
                if let Some(open) = &open {
                    :hidden=$(!open.get())
                }
            >
                (child)
            </div>
        </div>
    })
}

#[component]
async fn owl() -> Result<impl View> {
    Ok(view! {
        // Beside what it says, or labelled, so it needs no alt text.
        <img
            src="/static/wst_logo.png"
            width="800"
            height="800"
            decoding="async"
            alt=""
        >
    })
}
/// A step's title and what to do.
#[component]
async fn step_text(step: OnboardingStep) -> Result<impl View> {
    Ok(view! {
        match step {
            // The home page's, not the card's.
            OnboardingStep::Welcome => {

            }
            OnboardingStep::CreateDataSource => {
                card_title("Add a data source")
                card_description(
                    "Tell me where your media is by adding its directory on the "
                    <a class=(LINK) href=(href!(data_sources::page))>"Data Sources"</a>
                    " page."
                )
            }
            OnboardingStep::AwaitCatalog => {
                card_title("Wait for the catalog")
                card_description(
                    "An ingest reads every file, then an interpretation identifies them. "
                    "Follow along on the "
                    <a class=(LINK) href=(href!(ingests::page))>"Ingests"</a>
                    " page. A big library can take a while."
                )
            }
            OnboardingStep::ReviewCatalog => {
                card_title("Review the catalog")
                card_description(
                    "Browse all identified works in the "
                    <a class=(LINK) href=(href!(catalog::page))>"Catalog"</a>
                    "."
                )
            }
            OnboardingStep::LearnCorrections => {
                card_title("Correct mistakes")
                card_description(
                    "If something is mislabeled, mark it to indicate it needs correction. "
                    "Marks show up on the "
                    <a class=(LINK) href=(href!(corrections::page))>"Corrections"</a>
                    " page, where you can make bulk assertions to override incorrect claims."
                )
            }
            OnboardingStep::CreateView => {
                card_title("Create a view")
                card_description(
                    "A view is a directory of links to your media, named so a media server "
                    "like Jellyfin or Plex can identify it. Add one on the "
                    <a class=(LINK) href=(href!(views::page))>"Views"</a>
                    " page and add that directory to your media server's library."
                )
            }
        }
    })
}
