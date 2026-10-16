use crate::app::page_help::page_help;
use crate::icons::{CHECK, COPY, EYE, EYE_OFF, lucide};
use crate::{
    context::app,
    styles::{MONO, SECTION_H1},
    ui::{
        badge::{BadgeVariant, badge},
        button::{ButtonSize, ButtonVariant, button},
        card::{card, card_content, card_description, card_footer, card_header, card_title},
    },
};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::Form,
        error::{SeeOther, see_other},
        href, page, route,
    },
    runtime::{Event, signal},
    view::{View, attributes, view},
};
use wanshitong_app::{
    SHARED_TMDB_TOKEN,
    onboarding::{self, OnboardingSetBy, OnboardingStep},
};

#[derive(Deserialize)]
struct OnboardingForm {
    enabled: bool,
}

/// The effective configuration: the settings, and the whole thing as TOML to
/// copy. Also the first-run guide's switch, the one setting kept in the
/// database, unless the config file sets it.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let config = app(cx).config();
    let shared_token = config.tmdb_token.as_deref() == Some(SHARED_TMDB_TOKEN);
    // Without the shared token, which is only the default, so that a copy
    // used as a config file doesn't pin it.
    let toml = if shared_token {
        let mut config = config.clone();
        config.tmdb_token = None;
        toml::to_string_pretty(&config)?
    } else {
        toml::to_string_pretty(config)?
    };
    let guide = onboarding::get_onboarding(app(cx)).await?;
    // Whether the TMDB token is showing, rather than masked.
    let shown = signal(cx, || false);
    // What the guide's button does, if the UI decides whether it's shown.
    let guide_action = match (guide.set_by, guide.enabled, guide.step) {
        (OnboardingSetBy::ConfigFile, _, _) => None,
        (_, false, _) => Some((true, "Show guide")),
        (_, true, Some(_)) => Some((false, "Hide guide")),
        (_, true, None) => Some((true, "Start over")),
    };

    Ok(view! {
        <div class="flex items-center gap-4">
            <h1 class=(SECTION_H1)>"Config"</h1>
            page_help(
                section: true,
                <p>
                    "The settings Wan Shi Tong is running with. They're read at startup "
                    "from "
                    <code class=(MONO) translate="no">"config.toml"</code>
                    " in the config "
                    "directory, with "
                    <code class=(MONO) translate="no">"WST__"</code>
                    " environment "
                    "variables taking precedence. Changes apply after a restart; ingests "
                    "that already ran keep the probing settings they ran with."
                </p>
            )

            // Copies the config as TOML. Hidden until Alpine starts, as it needs it.
            button(
                variant: ButtonVariant::Outline,
                size: ButtonSize::Sm,
                attrs: attributes! {
                    class="ml-auto"
                    type="button"
                    style="display: none"
                    x-data="copyButton"
                    data-copy=(toml)
                    x-show="true"
                    x-on:click="copy()" // Defined in ts/copy.ts.
                },
                lucide(
                    icon: COPY,
                    attrs: attributes! { class="size-4" x-show="!copied" }
                )
                lucide(
                    icon: CHECK,
                    attrs: attributes! { class="size-4" x-show="copied" style="display: none" }
                )
                <span x-text="copied ? 'Copied' : 'Copy TOML'">"Copy TOML"</span>
            )
        </div>

        card(
            card_header(
                card_title("Web server")
                card_description("Where Wan Shi Tong listens for HTTP.")
            )
            card_content(
                <dl class="kv-list">
                    <div>
                        <dt>"Host"</dt>
                        <dd class=(MONO) translate="no">(config.host.to_string())</dd>
                    </div>

                    <div>
                        <dt>"Port"</dt>
                        <dd class=(MONO) translate="no">(config.port.to_string())</dd>
                    </div>
                </dl>
            )
        )

        card(
            card_header(
                card_title("TMDB")
                card_description("Movie metadata from The Movie Database.")
            )
            card_content(
                <dl class="kv-list">
                    <div>
                        <dt>"API token"</dt>
                        <dd>
                            if shared_token {
                                badge(variant: BadgeVariant::Outline, "Shared")
                                <span class="ml-2 text-muted-foreground">
                                    "Wan Shi Tong's shared token, free for noncommercial use. Set "
                                    <code class=(MONO) translate="no">"tmdb_token"</code>
                                    " in the config file to use yours."
                                </span>
                            } else if let Some(token) = &config.tmdb_token {
                                // Masked until asked for. The text fills the
                                // row, padded to the button's height, so the
                                // button stays put at the row's end however
                                // long the token is.
                                <div class="flex items-start gap-2">
                                    <div class="min-w-0 flex-1 py-2.5">
                                        <span class=(MONO) :hidden=$(shown.get())>
                                            "••••••••"
                                        </span>
                                        <span
                                            class=(format!("{MONO} break-all"))
                                            translate="no"
                                            :hidden=$(!shown.get())
                                        >
                                            (token)
                                        </span>
                                    </div>
                                    button(
                                        variant: ButtonVariant::Ghost,
                                        size: ButtonSize::Icon,
                                        attrs: attributes! {
                                            class="shrink-0"
                                            type="button"
                                            :aria-label=$(if shown.get() {
                                                "Hide token"
                                            } else {
                                                "Show token"
                                            })
                                            @click=$(|_e: Event| shown.toggle())
                                        },
                                        lucide(
                                            icon: EYE,
                                            attrs: attributes! { class="size-4" :hidden=$(shown.get()) }
                                        )
                                        lucide(
                                            icon: EYE_OFF,
                                            attrs: attributes! { class="size-4" :hidden=$(!shown.get()) }
                                        )
                                    )
                                </div>
                            } else {
                                badge(variant: BadgeVariant::Outline, "Not set")
                            }
                        </dd>
                    </div>
                </dl>
            )
        )

        card(
            card_header(
                card_title("Probing")
                card_description("How ingests measure files, with ffprobe from FFmpeg.")
            )
            card_content(
                <dl class="kv-list">
                    <div>
                        <dt>"ffprobe"</dt>
                        <dd>
                            <span class=(format!("{MONO} break-all")) translate="no">
                                (&config.ffprobe_path)
                            </span>
                            if !config.ffprobe_path.contains(std::path::is_separator) {
                                <span class="ml-2 text-muted-foreground">
                                    "Found on the service's "
                                    <code class=(MONO) translate="no">"PATH"</code>
                                    ". Set "
                                    <code class=(MONO) translate="no">"ffprobe_path"</code>
                                    " in the config file to run one elsewhere."
                                </span>
                            }
                        </dd>
                    </div>

                    <div>
                        <dt>"Arguments"</dt>
                        <dd class=(format!("{MONO} break-all")) translate="no">
                            (&config.ffprobe_command)
                        </dd>
                    </div>
                </dl>
            )
        )

        card(
            card_header(
                card_title("First-run guide")
                card_description("A quick tutorial of the app's basic features.")
            )
            card_content(
                <dl class="kv-list">
                    <div>
                        <dt>"Shown"</dt>
                        <dd>
                            if guide.enabled {
                                badge(variant: BadgeVariant::Success, "On")
                            } else {
                                badge(variant: BadgeVariant::Outline, "Off")
                            }
                            if guide.set_by == OnboardingSetBy::ConfigFile {
                                <span class="ml-2 text-muted-foreground">
                                    "Set by "
                                    <code class=(MONO) translate="no">"onboarding"</code>
                                    " in the config file."
                                </span>
                            }
                        </dd>
                    </div>

                    <div>
                        <dt>"Progress"</dt>
                        <dd>
                            (match guide.step.map(OnboardingStep::number) {
                                Some(Some(number)) => format!(
                                    "Step {number} of {}",
                                    OnboardingStep::NUMBERED,
                                ),
                                Some(None) => "Not started".to_string(),
                                None => "Finished".to_string(),
                            })
                        </dd>
                    </div>
                </dl>
            )
            if let Some((enabled, label)) = guide_action {
                card_footer(
                    <form method="post" action=(href!(set_onboarding))>
                        <input type="hidden" name="enabled" value=(enabled.to_string())>
                        button(
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            attrs: attributes! { type="submit" },
                            (label)
                        )
                    </form>
                )
            }
        )
    })
}

/// Shows or hides the first-run guide, starting it over if it was finished.
#[route(POST "./onboarding")]
async fn set_onboarding(cx: &Cx, Form(form): Form<OnboardingForm>) -> Result<SeeOther> {
    onboarding::set_onboarding_enabled(app(cx), form.enabled).await?;
    Ok(see_other(href!(page).resolve(cx)))
}
