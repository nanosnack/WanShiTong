//! NFO files: a media server's beside a video or at its series' root, and a
//! release's.

use super::*;

/// The NFO heuristic's claims about each file, by row ID.
fn nfo_claims(files: &[(PathId, FileFacts)]) -> Vec<(i64, ClaimPredicate)> {
    let claims = run_heuristics(files);
    (0..files.len())
        .flat_map(|subject| {
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter(|claim| claim.source == ClaimSource::Heuristic(Heuristic::Nfo))
                .map(move |claim| (files[subject].1.row_id.0, claim.pred.clone()))
        })
        .collect()
}

#[test]
fn a_movies_nfo_beside_it_names_the_movie() {
    let kodi = "<movie><title>Heat</title><year>1995</year><uniqueid type=\"tmdb\" default=\"true\">949</uniqueid><uniqueid type=\"imdb\">tt0113277</uniqueid></movie>";
    let files = [
        // Named after the video.
        video(1, "Heat (1995)/Heat (1995).mkv", 170.0),
        nfo(2, "Heat (1995)/Heat (1995).nfo", kodi),
        // Kodi's `movie.nfo`, the directory's only NFO, beside its only
        // video and a sample.
        video(3, "Alien3.1992-GRP/alien3.1992-grp.mkv", 145.0),
        video(4, "Alien3.1992-GRP/alien3.1992-grp-sample.mkv", 1.0),
        nfo(
            5,
            "Alien3.1992-GRP/movie.nfo",
            "<movie><title>Alien³</title><premiered>1992-05-22</premiered><tmdbid>8077</tmdbid></movie>",
        ),
        // A release's NFO, with its link, named after the release.
        video(6, "Blade.1998.1080p-GRP/Blade.1998.1080p-GRP.mkv", 120.0),
        nfo(
            7,
            "Blade.1998.1080p-GRP/blade.1998.1080p-grp.nfo",
            "GRP presents\n\nhttps://www.imdb.com/title/tt0120611/\n",
        ),
        // A pack's one NFO isn't each movie's.
        video(8, "Pack/Movie.One.2001.mkv", 100.0),
        video(9, "Pack/Movie.Two.2003.mkv", 100.0),
        nfo(10, "Pack/pack.nfo", "https://www.imdb.com/title/tt0000001/"),
        // An extra's is its main file's.
        video(11, "Heat (1995)/Extras/Trailer.mkv", 2.0),
        nfo(
            12,
            "Heat (1995)/Extras/Trailer.nfo",
            "<movie><uniqueid type=\"tmdb\">1</uniqueid></movie>",
        ),
    ];

    assert_eq!(
        nfo_claims(&files),
        vec![
            (1, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(949))),
            (
                1,
                ClaimPredicate::MovieExternalId(ExternalId::Imdb("tt0113277".into()))
            ),
            (1, ClaimPredicate::MovieTitle("Heat".into())),
            (1, ClaimPredicate::MovieReleaseYear(1995)),
            (1, ClaimPredicate::WorkKind(WorkKind::Movie)),
            (3, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(8077))),
            (3, ClaimPredicate::MovieTitle("Alien³".into())),
            (3, ClaimPredicate::MovieReleaseYear(1992)),
            (3, ClaimPredicate::WorkKind(WorkKind::Movie)),
            (
                6,
                ClaimPredicate::MovieExternalId(ExternalId::Imdb("tt0120611".into()))
            ),
        ]
    );

    // The NFO's title outranks the file name's, and the catalog goes by it.
    let catalog = catalog_without_kb(&files, &[]);
    let titles: Vec<String> = movies(&catalog).into_iter().map(|(t, _)| t).collect();
    assert!(titles.contains(&"Heat".to_string()), "{titles:?}");
    assert!(titles.contains(&"Alien³".to_string()), "{titles:?}");
}

#[test]
fn a_series_nfos_name_its_episodes_and_the_show() {
    let show = "<tvshow><title>Breaking Bad</title><year>2008</year><uniqueid type=\"tvdb\" default=\"true\">81189</uniqueid><uniqueid type=\"tmdb\">1396</uniqueid></tvshow>";
    let files = [
        nfo(1, "Breaking Bad/tvshow.nfo", show),
        // An episode by its name, with its own NFO.
        video(2, "Breaking Bad/Season 5/Breaking Bad S05E14.mkv", 47.0),
        nfo(
            3,
            "Breaking Bad/Season 5/Breaking Bad S05E14.nfo",
            "<episodedetails><title>Ozymandias</title><showtitle>Breaking Bad</showtitle><season>5</season><episode>14</episode></episodedetails>",
        ),
        // An episode only by its NFO.
        video(4, "Breaking Bad/Season 5/Ozymandias.mkv", 47.0),
        nfo(
            5,
            "Breaking Bad/Season 5/Ozymandias.nfo",
            "<episodedetails><title>Ozymandias</title><season>5</season><episode>14</episode></episodedetails>",
        ),
        // A movie in the series' directory isn't the series'.
        video(6, "Breaking Bad/El Camino (2019).mkv", 122.0),
        // A season release's one NFO is the series', for each episode.
        video(7, "Berserk.S01.1080p-GRP/Berserk.S01E01.mkv", 24.0),
        video(8, "Berserk.S01.1080p-GRP/Berserk.S01E02.mkv", 24.0),
        nfo(
            9,
            "Berserk.S01.1080p-GRP/berserk.s01.1080p-grp.nfo",
            "https://www.imdb.com/title/tt0318871/",
        ),
    ];

    let series_ids = |row| {
        vec![
            (
                row,
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tvdb(81189)),
            ),
            (
                row,
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(1396)),
            ),
            (row, ClaimPredicate::TvSeriesName("Breaking Bad".into())),
            (row, ClaimPredicate::TvSeriesYear(2008)),
            (row, ClaimPredicate::WorkKind(WorkKind::TvSeries)),
        ]
    };
    let mut expected = vec![
        (2, ClaimPredicate::Season(5)),
        (2, ClaimPredicate::EpisodeRange([14, 14])),
        (2, ClaimPredicate::EpisodeTitle("Ozymandias".into())),
        (2, ClaimPredicate::TvSeriesName("Breaking Bad".into())),
        (2, ClaimPredicate::WorkKind(WorkKind::TvSeries)),
    ];
    expected.extend(series_ids(2));
    expected.extend([
        (4, ClaimPredicate::Season(5)),
        (4, ClaimPredicate::EpisodeRange([14, 14])),
        (4, ClaimPredicate::EpisodeTitle("Ozymandias".into())),
        (4, ClaimPredicate::WorkKind(WorkKind::TvSeries)),
    ]);
    expected.extend(series_ids(4));
    expected.extend([
        (
            7,
            ClaimPredicate::TvSeriesExternalId(ExternalId::Imdb("tt0318871".into())),
        ),
        (
            8,
            ClaimPredicate::TvSeriesExternalId(ExternalId::Imdb("tt0318871".into())),
        ),
    ]);
    assert_eq!(nfo_claims(&files), expected);

    // The video named only by its NFO is cataloged as an episode, with the
    // one named by its marker, under the name the file names outrank the
    // NFO's with.
    let catalog = catalog_without_kb(&files, &[]);
    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|s| (s.name.clone(), s.episodes.len()))
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![("berserk".to_string(), 2), ("breaking bad".to_string(), 2)]
    );
}

#[test]
fn a_discs_nfo_is_beside_it_or_in_its_folder() {
    let files = [
        disc(
            1,
            "Heat (1995)/BDMV/index.bdmv",
            DiscKind::BluRay,
            "Heat (1995)",
            170.0,
        ),
        nfo(
            2,
            "Heat (1995)/movie.nfo",
            "<movie><uniqueid type=\"tmdb\">949</uniqueid></movie>",
        ),
        disc(3, "Blade.iso", DiscKind::BluRay, "Blade.iso", 120.0),
        nfo(
            4,
            "Blade.nfo",
            "<movie><uniqueid type=\"tmdb\">36586</uniqueid></movie>",
        ),
    ];
    assert_eq!(
        nfo_claims(&files),
        vec![
            (1, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(949))),
            (1, ClaimPredicate::WorkKind(WorkKind::Movie)),
            (3, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(36586))),
            (3, ClaimPredicate::WorkKind(WorkKind::Movie)),
        ]
    );
}

#[test]
fn an_nfos_id_outranks_one_in_a_name_but_not_an_assertion() {
    let files = [
        video(1, "Heat (1995) {tmdb-1}/Heat.mkv", 170.0),
        nfo(
            2,
            "Heat (1995) {tmdb-1}/Heat.nfo",
            "<movie><uniqueid type=\"tmdb\">949</uniqueid></movie>",
        ),
    ];
    let data_source = files[0].0.data_source;
    let asserted = assertion(
        data_source,
        &["Heat (1995) {tmdb-1}/Heat.mkv"],
        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(2)),
    );

    let catalog = catalog_without_kb(&files, &[]);
    let ids: Vec<_> = active_claims(&catalog, 1)
        .into_iter()
        .filter(|pred| matches!(pred, ClaimPredicate::MovieExternalId(_)))
        .collect();
    assert_eq!(
        ids,
        vec![ClaimPredicate::MovieExternalId(ExternalId::Tmdb(949))]
    );

    let catalog = catalog_without_kb(&files, &[asserted]);
    let ids: Vec<_> = active_claims(&catalog, 1)
        .into_iter()
        .filter(|pred| matches!(pred, ClaimPredicate::MovieExternalId(_)))
        .collect();
    assert_eq!(
        ids,
        vec![ClaimPredicate::MovieExternalId(ExternalId::Tmdb(2))]
    );
}
