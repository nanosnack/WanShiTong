//! Assertions, and how they outrank heuristics and knowledge bases.

use super::*;

#[test]
fn unidentified_videos_are_the_videos_of_no_identified_work() {
    let media = DataSourceUuid(Uuid::nil());
    let files = [
        // Identified, with its extra.
        video(1, "Redline (2009)/Redline.mkv", 102.8),
        video(2, "Redline (2009)/Extras/Trailer.mkv", 2.0),
        // A movie by its name, but not identified.
        video(3, "Obscure Film (1999)/Obscure Film (1999).mkv", 90.0),
        // Claimed to be a sample, which is only a guess.
        video(4, "Obscure Film (1999)/Sample/sample.mkv", 0.5),
        // Not videos.
        file(5, "Obscure Film (1999)/notes.txt"),
        {
            let (path_id, mut facts) = video(6, "Obscure Film (1999)/Scans/1.jpg", 0.0);
            facts.mime_type = Some("image/jpeg".into());
            (path_id, facts)
        },
    ];
    let assertions = [assertion(
        media,
        &["Redline (2009)/Redline.mkv"],
        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(71883)),
    )];
    let catalog = catalog_without_kb(&files, &assertions);

    let mut unidentified: Vec<_> = catalog.unidentified_videos.iter().map(|id| id.0).collect();
    unidentified.sort();
    assert_eq!(unidentified, [3, 4]);
}

#[test]
fn assertions_outrank_heuristics_on_the_file_they_name() {
    let media = DataSourceUuid(Uuid::nil());
    let files = [
        video(1, "Redline ITBD Remux.mkv", 102.8),
        video(2, "Redline.2009.1080p.mkv", 102.8),
    ];
    let tmdb = |id| ClaimPredicate::MovieExternalId(ExternalId::Tmdb(id));
    let assertions = [
        assertion(media, &["Redline ITBD Remux.mkv"], tmdb(71883)),
        assertion(
            media,
            &["Redline ITBD Remux.mkv"],
            ClaimPredicate::MovieTitle("Redline".into()),
        ),
        // The same path in another data source is another file.
        assertion(
            DataSourceUuid(Uuid::from_u128(1)),
            &["Redline.2009.1080p.mkv"],
            tmdb(1),
        ),
    ];
    let catalog = catalog_without_kb(&files, &assertions);

    let mut movies: Vec<_> = catalog
        .movies
        .iter()
        .map(|movie| (movie.title.clone(), movie.tmdb_id, movie.files[0].file_id.0))
        .collect();
    movies.sort();
    assert_eq!(
        movies,
        vec![
            ("Redline".into(), Some(71883), 1),
            ("redline".into(), None, 2),
        ]
    );
}

#[tokio::test]
async fn assertions_outrank_what_a_knowledge_base_says_of_them() {
    let media = DataSourceUuid(Uuid::nil());
    let files = [video(1, "The Game Is Real.mkv", 30.0)];
    let assertions = [
        assertion(
            media,
            &["The Game Is Real.mkv"],
            ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1)),
        ),
        assertion(
            media,
            &["The Game Is Real.mkv"],
            ClaimPredicate::MovieTitle("The Game Is Real".into()),
        ),
    ];
    // What an asserted ID found, ranked above the assertion to outrank
    // heuristics: a series, which the assertions contradict, and a title
    // and year for the movie, of which only the year wasn't asserted.
    let above = Rank::Assertion + 2.0;
    let kb = FixedKb(vec![vec![
        Ranked::new(above, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1))),
        Ranked::new(above, ClaimPredicate::MovieTitle("Something Else".into())),
        Ranked::new(above, ClaimPredicate::MovieReleaseYear(2008)),
        Ranked::new(above, ClaimPredicate::TvSeriesName("Pride".into())),
    ]]);
    let catalog = catalog_with_kb(&files, &assertions, &kb).await;

    let movies: Vec<_> = catalog
        .movies
        .iter()
        .map(|movie| (movie.title.as_str(), movie.tmdb_id, movie.release_year))
        .collect();
    assert_eq!(movies, [("The Game Is Real", Some(1), Some(2008))]);
    assert!(catalog.tv_series.is_empty());
    // Every assertion is in effect.
    let asserted = active_claims(&catalog, 1)
        .into_iter()
        .filter(|pred| assertions.iter().any(|a| a.claim == *pred))
        .count();
    assert_eq!(asserted, 2);
}
