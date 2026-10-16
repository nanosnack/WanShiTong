use crate::app::page_help::page_help;
use crate::{
    app::{
        data_sources::data_source_id::{self, DataSourceId},
        draft, files,
    },
    context::app,
    display,
    styles::{H1, H2, LINK, MONO},
    ui::card::{card, card_content, card_header, card_title},
};
use topcoat::{
    Result,
    context::Cx,
    router::{error::RouterErrorExt, href, page, path_param},
    view::{View, component, view},
};
use uuid::Uuid;
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    domain::{MovieDetails, MovieUuid},
    queries,
};

path_param!(pub movie_id: Uuid, error = not_found);

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let id = MovieUuid(*path_param::<MovieId>(cx)?);
    let movie = queries::get_movie(app(cx), id).await?.ok_or_not_found()?;
    Ok(view! { movie_page(movie: movie) })
}

#[component]
async fn movie_page(cx: &Cx, movie: MovieDetails) -> Result<impl View> {
    let file_paths: Vec<_> = movie
        .files
        .iter()
        .map(|file| PathId::new(file.data_source_uuid, &file.rel_path))
        .collect();
    let here = href!(page, MovieId(movie.uuid.0)).resolve(cx);

    Ok(view! {
        <header class="flex flex-col gap-4 sm:flex-row sm:items-start">
            // TODO: cache posters on the backend
            if let Some(path) = &movie.tmdb_poster_path {
                <img
                    class="w-full max-w-xs shrink-0 rounded-lg shadow-sm sm:w-80"
                    src=(format!("https://image.tmdb.org/t/p/w780/{path}"))
                >
            }

            <div class="flex flex-col gap-2">
                <div class="flex items-center gap-2">
                    <h1 class=(H1) translate="no">(movie.title.as_str())</h1>
                    page_help(
                        <p>
                            "A movie in the catalog, with metadata from TMDB, "
                            "and all associated files. Open a file to see how it was "
                            "identified."
                        </p>
                        <p>
                            "If it's wrong, select its files and mark them, then "
                            "correct them on the Corrections page."
                        </p>
                    )
                </div>
                if let Some(release_year) = movie.release_year {
                    <p class="text-lg text-muted-foreground">
                        (release_year.to_string())
                    </p>
                }
                if let Some(tagline) = &movie.tagline {
                    <p class="italic text-muted-foreground">(tagline.as_str())</p>
                }
                if let Some(overview) = &movie.overview {
                    <p class="max-w-prose text-sm">(overview.as_str())</p>
                }
            </div>
        </header>

        draft::pending_in_draft(files: file_paths)

        card(
            card_header(card_title("Details"))
            card_content(
                <dl class="kv-list">
                    if let Some(original_title) = &movie.original_title {
                        <div>
                            <dt>"Original Title"</dt>
                            <dd translate="no">(original_title.as_str())</dd>
                        </div>
                    }

                    if let Some(original_language) = &movie.original_language {
                        <div>
                            <dt>"Original Language"</dt>
                            <dd translate="no">(original_language.as_str())</dd>
                        </div>
                    }

                    <div>
                        <dt>"UUID"</dt>
                        <dd class=(MONO) translate="no">(movie.uuid.to_string())</dd>
                    </div>

                    if let Some(id) = movie.tmdb_id {
                        <div>
                            <dt>"TMDB ID"</dt>
                            <dd>
                                <a
                                    class=(LINK)
                                    href=(format!("https://www.themoviedb.org/movie/{id}"))
                                    translate="no"
                                >
                                    (id.to_string())
                                </a>
                            </dd>
                        </div>
                    }

                    if let Some(id) = &movie.imdb_id {
                        <div>
                            <dt>"IMDB ID"</dt>
                            <dd>
                                <a
                                    class=(LINK)
                                    href=(format!("https://www.imdb.com/title/{id}"))
                                    translate="no"
                                >
                                    (id.to_string())
                                </a>
                            </dd>
                        </div>
                    }

                    <div>
                        <dt>"Data Source"</dt>
                        <dd>
                            <a
                                class=(LINK)
                                href=(href!(
                                    data_source_id::page,
                                    DataSourceId(movie.data_source_uuid.0),
                                ))
                                translate="no"
                            >
                                (movie.data_source_name.as_str())
                            </a>
                        </dd>
                    </div>

                    if let Some(duration) = movie.duration() {
                        <div>
                            <dt>"Duration"</dt>
                            <dd>(display::human_duration(duration))</dd>
                        </div>
                    }

                    if let Some(video) = movie
                        .media
                        .as_ref()
                        .and_then(|media| media.video.first()) {
                        <div>
                            <dt>"Video Codec"</dt>
                            <dd translate="no">(video.codec.to_string())</dd>
                        </div>

                        <div>
                            <dt>"Resolution"</dt>
                            <dd translate="no">
                                (format!("{}x{}", video.width, video.height))
                            </dd>
                        </div>
                    }
                </dl>
            )
        )

        <section class="flex flex-col gap-2">
            <h2 class=(H2)>"Files"</h2>
            if movie.files.is_empty() {
                <p class="text-muted-foreground">
                    "No files associated with this movie."
                </p>
            } else {
                files::work_files(work: movie.uuid.0, return_to: here)
            }
        </section>
    })
}
