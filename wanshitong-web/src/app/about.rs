use crate::{
    WST_VERSION,
    context::app,
    display,
    styles::{H1, MONO},
};
use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

/// The logo, which version of Wan Shi Tong this is and how long it's been up,
/// and the attribution TMDB requires.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let app = app(cx);

    Ok(view! {
        <header class="flex flex-col items-center gap-4 text-center">
            <img
                class="h-auto w-[400px] max-w-full"
                src="/static/wst_logo.png"
                width="800"
                height="800"
                decoding="async"
                alt="Wan Shi Tong owl logo"
            >
            <h1 class=(H1) translate="no">"Wan Shi Tong 万事通"</h1>
        </header>

        <dl class="kv-list self-center">
            <div>
                <dt>"Version"</dt>
                <dd class=(MONO) translate="no">(WST_VERSION)</dd>
            </div>

            <div>
                <dt>"Uptime"</dt>
                <dd>(display::human_duration(app.uptime()))</dd>
            </div>

            <div>
                <dt>"Started"</dt>
                <dd class=(MONO) translate="no">
                    (display::ts(&app.started_at()).to_string())
                </dd>
            </div>
        </dl>

        // TMDB's API terms require its logo, less prominent than our own,
        // and this notice.
        <footer
            class="mt-6 flex flex-col items-center gap-3 text-center text-sm text-muted-foreground"
        >
            <a href="https://www.themoviedb.org" target="_blank" rel="noreferrer">
                <img
                    class="h-4 w-auto"
                    src="/static/tmdb_logo.svg"
                    alt="The Movie Database (TMDB)"
                >
            </a>
            <p>
                "This application uses TMDB and the TMDB APIs but is not endorsed, certified, "
                "or otherwise approved by TMDB."
            </p>
        </footer>
    })
}
