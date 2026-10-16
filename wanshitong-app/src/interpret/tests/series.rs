//! Series: their names and years, and which files are theirs.

use super::*;
use wanshitong_core::domain::Date;

#[test]
fn a_series_is_named_without_the_tags_around_its_marker() {
    let dir = "[Zork-Glorp] Suzumiya Haruhi-chan no Yuuutsu & Nyoroon Churuya-san [Wump_720p]";
    let files = [video(
        1,
        &format!(
            "{dir}/SPs/[Zork-Glorp] Suzumiya Haruhi-chan no Yuuutsu & Nyoroon Churuya-san [NCOP][Wump_720p][x265_flac].mkv"
        ),
        1.5,
    )];
    let claims = run_heuristics(&files);

    // Not its name with long vowels written short, a last resort.
    let short = ClaimSource::Heuristic(Heuristic::TitleWithShortVowels);
    let names: Vec<_> = claims[&0]
        .iter()
        .filter(|claim| claim.source != short)
        .filter_map(|claim| match &claim.pred {
            ClaimPredicate::TvSeriesName(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        vec!["suzumiya haruhi-chan no yuuutsu & nyoroon churuya-san"]
    );
}

#[test]
fn episodes_are_named_after_their_series() {
    let cg = "[Foo] Code Geass (Seasons 1-2) [BD 1080p]/[Foo] Code Geass S1";
    let files = [
        video(1, &format!("{cg}/[Foo] Code Geass - S01E01.mkv"), 24.0),
        video(2, &format!("{cg}/[Foo] Code Geass - S01E02.mkv"), 24.0),
        // Only the directory names the series, and not its season one.
        video(3, "The Wire/Season 1/S01E03 - The Buys.mkv", 60.0),
        video(4, "Doctor.Who.2005.S02E01.720p.mkv", 45.0),
        // A directory of episodes isn't a movie's.
        video(5, "Code Geass S2/Code Geass - S02E01.mkv", 24.0),
        video(6, "Code Geass S2/Code Geass - S02E02.mkv", 24.0),
        // Subtitles aren't episodes.
        file(7, &format!("{cg}/[Foo] Code Geass - S01E01.en.srt")),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series
                .episodes
                .iter()
                .map(|e| (e.file_id.0, e.season_number, e.episode_range))
                .collect();
            episodes.sort();
            (series.name.as_str(), episodes)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            (
                "code geass",
                vec![
                    (1, Some(1), [1, 1]),
                    (2, Some(1), [2, 2]),
                    (5, Some(2), [1, 1]),
                    (6, Some(2), [2, 2])
                ]
            ),
            ("doctor who", vec![(4, Some(2), [1, 1])]),
            ("the wire", vec![(3, Some(1), [3, 3])]),
        ]
    );
    assert_eq!(movies(&catalog), vec![]);
}

#[test]
fn a_creditless_ending_names_its_series_without_the_season() {
    let s2 = "One Punch Man (2015) [tbdbid-293088]/[bar] One Punch Man S2 [BD 1080p FLAC]";
    let files = [
        video(
            1,
            &format!("{s2}/[bar] One Punch Man S2 - 01 [BD 1080p FLAC] [41F7C8E6].mkv"),
            23.7,
        ),
        video(
            2,
            &format!("{s2}/[bar] One Punch Man S2 - 02 [BD 1080p FLAC] [7F3B1E0D].mkv"),
            23.7,
        ),
        video(
            3,
            &format!("{s2}/[bar] One Punch Man S2 - 03 [BD 1080p FLAC] [1D63E8A4].mkv"),
            23.7,
        ),
        video(
            4,
            &format!("{s2}/Extras/[bar] One Punch Man S2 - NCED 01 [BD 1080p FLAC] [94C8D6A3].mkv"),
            1.5,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            let extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            (series.name.as_str(), episodes.len(), extras)
        })
        .collect();
    assert_eq!(series, vec![("one punch man", 3, vec![4])]);
}

#[test]
fn a_directory_names_a_series_before_its_season() {
    let mob = "Mob.Psycho";
    let s1 = "Mob Psycho 100/Mob Psycho 100 [2016] COMPLETE 1080p BDRip x265 PCM 2.0 Blarg [Flob]";
    let s2 = "Mob Psycho 100 S02+OVA 1080p Dual Audio BD Remux-Quux";
    let curb = "Curb Your Enthusiasm Season 1, 2, 3, 4, 5, 6, 7 & 8 + Extras DVDRip Zib";
    let files = [
        video(
            1,
            &format!("{mob}/{s1}/Mob Psycho 100 - 01 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv"),
            24.2,
        ),
        video(
            2,
            &format!("{mob}/{s1}/Mob Psycho 100 - 02 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv"),
            24.2,
        ),
        video(
            3,
            &format!("{mob}/{s1}/Mob Psycho 100 - 03 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv"),
            24.2,
        ),
        // Named by release directories, before their season.
        video(
            4,
            &format!(
                "{mob}/{s2}/Mob Psycho 100 S02 1080p Dual BD Remux-Quux/S02E01-Ripped Apart ~Someone Is Watching.mkv"
            ),
            24.2,
        ),
        video(
            5,
            &format!(
                "{mob}/{s2}/Mob Psycho 100 S02 1080p Dual BD Remux-Quux/S02E02-Urban Legends ~Encountering Rumors.mkv"
            ),
            24.2,
        ),
        video(
            7,
            &format!("{curb}/Season 1/S01E01 - The Pants Tent.avi"),
            28.0,
        ),
        // Its name ends with a number, not a season.
        video(8, "Blake's 7/Season 1/S01E01 - The Way Back.mkv", 50.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![
            ("blakes 7".into(), vec![(8, Some(1), [1, 1])]),
            ("curb your enthusiasm".into(), vec![(7, Some(1), [1, 1])]),
            (
                "mob psycho 100".into(),
                vec![
                    (1, None, [1, 1]),
                    (2, None, [2, 2]),
                    (3, None, [3, 3]),
                    (4, Some(2), [1, 1]),
                    (5, Some(2), [2, 2]),
                ]
            ),
        ]
    );
}

#[tokio::test]
async fn an_episode_outranks_a_movie_found_only_by_its_title() {
    let dir = "DAN.DA.DAN/DAN DA DAN (2024) S02 (1080p CR WEB-DL H264 AAC 2.0 Dual-Audio) [Wubble]";
    let files = [video(
        1,
        &format!(
            "{dir}/DAN DA DAN (2024) S02E01v2 (1080p CR WEB-DL H264 AAC 2.0 Dual-Audio) [Wubble].mkv"
        ),
        24.2,
    )];
    // As high as a movie found by a container title tag (2.0) ranks
    // without a runtime match: with the TMDB, release year and title
    // bonuses.
    let rank = 2.0 + 4.5;
    let kb = FixedKb(vec![vec![
        Ranked::new(
            rank,
            ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1308681)),
        ),
        Ranked::new(
            rank,
            ClaimPredicate::MovieTitle("DAN DA DAN: First Encounter".into()),
        ),
    ]]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    assert_eq!(
        series_episodes(&catalog),
        vec![("dan da dan".into(), vec![(1, Some(2), [1, 1])])]
    );
    assert_eq!(movies(&catalog), vec![]);
}

#[test]
fn episodes_of_a_series_are_one_series() {
    let media = DataSourceUuid(Uuid::nil());
    let files = [
        video(1, "Code Geass - S01E01.mkv", 24.0),
        video(2, "Code Geass - S01E02.mkv", 24.0),
        video(3, "Code Geass - S02E01.mkv", 24.0),
        video(4, "The Wire - S01E01.mkv", 60.0),
        video(5, "Wire S01E02.mkv", 60.0),
    ];
    let name = |name: &str| ClaimPredicate::TvSeriesName(name.into());
    let tmdb = ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(1438));
    let assertions = [
        // Named alike, without IDs.
        assertion(
            media,
            &[
                "Code Geass - S01E01.mkv",
                "Code Geass - S01E02.mkv",
                "Code Geass - S02E01.mkv",
            ],
            name("Code Geass"),
        ),
        // Named differently, with an ID in common.
        assertion(media, &["The Wire - S01E01.mkv"], name("The Wire")),
        assertion(media, &["Wire S01E02.mkv"], name("Wire")),
        assertion(media, &["The Wire - S01E01.mkv", "Wire S01E02.mkv"], tmdb),
    ];
    let catalog = catalog_without_kb(&files, &assertions);

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series
                .episodes
                .iter()
                .map(|e| (e.file_id.0, e.season_number, e.episode_range))
                .collect();
            episodes.sort();
            (series.name.as_str(), series.tmdb_id, episodes)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            (
                "Code Geass",
                None,
                vec![
                    (1, Some(1), [1, 1]),
                    (2, Some(1), [2, 2]),
                    (3, Some(2), [1, 1])
                ]
            ),
            (
                "The Wire",
                Some(1438),
                vec![(4, Some(1), [1, 1]), (5, Some(1), [2, 2])]
            ),
        ]
    );
    assert!(catalog.movies.is_empty());
}

#[tokio::test]
async fn a_series_is_told_from_another_of_its_name_by_its_year() {
    let files = [
        // The year in the file name.
        video(
            1,
            "Hunter X Hunter (2011)/Season 1/Hunter x Hunter (2011) - S01E01 (001) - Departure x And x Friends.mkv",
            23.0,
        ),
        video(
            2,
            "ONE.PIECE.2023.S02.2160p.NF.WEB-DL.DDP5.1.Atmos.DV.HDR.H.265-Xyzzy/ONE.PIECE.2023.S02E01.2160p.NF.WEB-DL.DDP5.1.Atmos.DV.HDR.H.265-Xyzzy.mkv",
            55.0,
        ),
        // The year in the name of a directory above.
        video(
            3,
            "Alfred Hitchcock Presents (1955)/Alfred Hitchcock Presents Season 1/Alfred Hitchcock Presents - S01E01 - Revenge.avi",
            25.0,
        ),
        video(
            4,
            "One Piece (1999) [tvdbid-81797]/Arc 12 - Arabasta/One.Piece.E92.1080p.mkv",
            20.5,
        ),
    ];
    // Each name's other series first, as TMDB lists them.
    let kb = series_kb(vec![
        (45952, "Hunter x Hunter", 1999),
        (46298, "Hunter x Hunter", 2011),
        (2751, "The New Alfred Hitchcock Presents", 1985),
        (5378, "Alfred Hitchcock Presents", 1955),
        (37854, "One Piece", 1999),
        (111110, "ONE PIECE", 2023),
    ]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let files: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            (series.tmdb_id, files)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            (Some(5378), vec![3]),
            (Some(37854), vec![4]),
            (Some(46298), vec![1]),
            (Some(111110), vec![2]),
        ]
    );
}

#[tokio::test]
async fn an_asserted_series_id_outranks_the_series_its_name_finds() {
    let media = DataSourceUuid(Uuid::nil());
    let first = "Hunter x Hunter/Season 1/Hunter x Hunter - S01E01.mkv";
    let second = "Hunter x Hunter/Season 1/Hunter x Hunter - S01E02.mkv";
    let files = [video(1, first, 23.0), video(2, second, 23.0)];
    // The name finds the 1999 series first, as TMDB lists them.
    let kb = series_kb(vec![
        (45952, "Hunter x Hunter", 1999),
        (46298, "Hunter x Hunter", 2011),
    ]);
    let tmdb = ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(46298));
    let assertions = [assertion(media, &[first, second], tmdb)];
    let catalog = catalog_with_kb(&files, &assertions, &kb).await;

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let files: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            (series.tmdb_id, files)
        })
        .collect();
    assert_eq!(series, vec![(Some(46298), vec![1, 2])]);
}

/// An episode's release name, e.g. `Babylon.Berlin.S02E01.BluRay.1080i`, leaves
/// titles that no work has, and a series search for any title of an episode
/// finds its series no better than its name does. So a knowledge base is asked
/// for an episode's series by its name alone, and for a movie only by a title
/// that could be one's.
#[tokio::test]
async fn an_episode_with_a_series_name_is_searched_for_by_the_name_alone() {
    let files = [
        video(
            1,
            "Babylon Berlin/Babylon.Berlin.S02E01.BluRay.1080i.Remux-QuUxTeR.mkv",
            45.0,
        ),
        // Numbered by its season's directory, with a title a movie could have.
        video(2, "Babylon Berlin/Season 2/Babylon Berlin 202.mkv", 45.0),
    ];
    let kb = series_kb(vec![(66980, "Babylon Berlin", 2017)]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let searches = |file_id: i64| -> Vec<String> {
        let (_, log) = catalog
            .audit_logs
            .iter()
            .find(|(id, _)| id.0 == file_id)
            .unwrap();
        log.kb_queries
            .iter()
            .filter_map(|query| match query {
                KbQueryEvent::MovieSearch { query, .. } => Some(format!("movie {query}")),
                KbQueryEvent::TvSeriesSearch { query, .. } => Some(format!("series {query}")),
                _ => None,
            })
            .collect()
    };
    assert_eq!(searches(1), ["series babylon berlin"]);
    assert_eq!(
        searches(2),
        ["movie babylon berlin 202", "series babylon berlin"]
    );
    let series: Vec<_> = catalog.tv_series.iter().map(|s| s.tmdb_id).collect();
    assert_eq!(series, [Some(66980)]);
}

#[tokio::test]
async fn a_title_that_finds_no_movie_is_looked_up_as_a_series() {
    let files = [
        // A season's whole release named without its episodes' numbers.
        video(
            1,
            "[Blorp] Glimmer Peaks [BD 1080p]/[Blorp] Glimmer Peaks.mkv",
            24.0,
        ),
        // Words that are only numbered, e.g. a concert's day and act: no
        // series to look for.
        video(2, "Concerts/Day1 Act2.mkv", 140.0),
    ];
    let kb = CatalogKb {
        series: vec![
            kb_series(301, "Glimmer Peaks", Some(2018)),
            kb_series(302, "Day1 Act2 Night", Some(2001)),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let series: Vec<(Option<u32>, Vec<i64>)> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut files: Vec<i64> = series.file_ids().map(|id| id.0).collect();
            files.sort();
            (series.tmdb_id, files)
        })
        .collect();
    assert_eq!(series, vec![(Some(301), vec![1])]);
}

#[tokio::test]
async fn a_romanized_name_is_looked_up_with_long_vowels_written_short() {
    let files = [
        // Listed under an English name, and romanized with its long vowels
        // written short.
        video(1, "Dramas/Zibble no Fnordoujo 03.mkv", 45.0),
        // Found so by another name, which isn't the file's.
        video(2, "Dramas/Wumbo no Fnordoujo 02.mkv", 45.0),
    ];
    let kb = CatalogKb {
        series: vec![
            kb_series(401, "Clinic of Zibble", Some(2013)),
            kb_series(402, "Wumbo Tales", Some(2016)),
        ],
        alternative_titles: vec![
            (401, "Zibble no Fnordojo"),
            (402, "Wumbo no Fnordojo Gaiden"),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let mut series: Vec<(Option<u32>, Vec<i64>)> = catalog
        .tv_series
        .iter()
        .map(|series| (series.tmdb_id, series.file_ids().map(|id| id.0).collect()))
        .collect();
    series.sort();
    assert_eq!(series, vec![(None, vec![2]), (Some(401), vec![1])]);
}

/// Two words for each of 16 videos, alike for none.
const SIXTEEN_NAMES: [&str; 16] = [
    "glorp fnord",
    "wumbo lark",
    "zibble road",
    "yonder peak",
    "blorp night",
    "fnord tide",
    "lark song",
    "road home",
    "peak snow",
    "night rain",
    "tide pool",
    "song bird",
    "home run",
    "snow fall",
    "rain dance",
    "bird call",
];

#[tokio::test]
async fn a_batchs_episodes_are_not_searched_for_by_their_own_titles() {
    // However few: a season of six is a batch as much as a series' run.
    let files: Vec<_> = (1..=6)
        .map(|n| {
            let name = SIXTEEN_NAMES[n - 1];
            video(
                n as i64,
                &format!("Zibble Saga/Zibble Saga - {name}.mkv"),
                24.0,
            )
        })
        .collect();
    let kb = CatalogKb {
        series: vec![kb_series(501, "Zibble Saga", Some(2010))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    // Only the first few, whose titles find nothing, are searched for so.
    let searched: HashSet<&str> = catalog
        .audit_logs
        .iter()
        .flat_map(|(_, log)| &log.kb_queries)
        .filter_map(|query| match query {
            KbQueryEvent::MovieSearch { query, .. } => Some(query.as_str()),
            _ => None,
        })
        .filter(|query| SIXTEEN_NAMES.iter().any(|name| query.ends_with(name)))
        .collect();
    assert_eq!(searched.len(), 3, "{searched:?}");
    let series: Vec<(Option<u32>, usize)> = catalog
        .tv_series
        .iter()
        .map(|series| (series.tmdb_id, series.file_ids().count()))
        .collect();
    assert_eq!(series, vec![(Some(501), 6)]);
}

#[tokio::test]
async fn a_directory_of_movies_has_each_searched_for_by_its_title() {
    // Movies by their durations, though their names say nothing of it: no
    // batch.
    let files: Vec<_> = (1..=16)
        .map(|n| {
            let name = SIXTEEN_NAMES[n - 1];
            video(
                n as i64,
                &format!("Collection/Zibble Saga - {name}.mkv"),
                100.0,
            )
        })
        .collect();
    let kb = CatalogKb {
        movies: (1..=16)
            .map(|n| {
                let title = format!("Zibble Saga: {}", SIXTEEN_NAMES[n - 1]);
                kb_movie(600 + n as u32, &title, Some(2000 + n as u32), None)
            })
            .collect(),
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let identified = catalog
        .movies
        .iter()
        .filter(|movie| movie.tmdb_id.is_some())
        .count();
    assert_eq!(identified, 16);
}

#[tokio::test]
async fn of_series_named_alike_the_one_with_the_files_season_is_chosen() {
    let files = [video(1, "Wumbo Lark S04/Wumbo.Lark.S04E02.720p.mkv", 24.0)];
    let seasons = |count: u32| -> Vec<SeasonSummary> {
        (1..=count)
            .map(|number| SeasonSummary {
                number,
                name: format!("Season {number}"),
                episode_count: 10,
            })
            .collect()
    };
    let kb = CatalogKb {
        series: vec![
            // Listed first, e.g. a remake, but it ran two seasons.
            SeriesRecord {
                seasons: seasons(2),
                ..kb_series(701, "Wumbo Lark", Some(2015))
            },
            SeriesRecord {
                seasons: seasons(6),
                ..kb_series(702, "Wumbo Lark", Some(1992))
            },
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let series: Vec<Option<u32>> = catalog.tv_series.iter().map(|s| s.tmdb_id).collect();
    assert_eq!(series, vec![Some(702)]);
}

#[tokio::test]
async fn of_series_named_alike_the_one_whose_season_is_as_long_as_the_release_is_chosen() {
    // A season pack of three, and in another release, two of a season of
    // ten: too few to tell by.
    let files = [
        video(1, "Wumbo Lark S01/Wumbo.Lark.S01E01.mkv", 60.0),
        video(2, "Wumbo Lark S01/Wumbo.Lark.S01E02.mkv", 60.0),
        video(3, "Wumbo Lark S01/Wumbo.Lark.S01E03.mkv", 60.0),
        video(4, "Glimmer S02/Glimmer.S02E01.mkv", 45.0),
        video(5, "Glimmer S02/Glimmer.S02E02.mkv", 45.0),
    ];
    let seasons = |episode_count: u32| -> Vec<SeasonSummary> {
        (1..=2)
            .map(|number| SeasonSummary {
                number,
                name: format!("Season {number}"),
                episode_count,
            })
            .collect()
    };
    let kb = CatalogKb {
        series: vec![
            // Listed first, e.g. the better known, but its seasons are long.
            SeriesRecord {
                seasons: seasons(13),
                ..kb_series(701, "Wumbo Lark", Some(1971))
            },
            SeriesRecord {
                seasons: seasons(3),
                ..kb_series(702, "Wumbo Lark", Some(2010))
            },
            SeriesRecord {
                seasons: seasons(10),
                ..kb_series(801, "Glimmer", Some(2005))
            },
            SeriesRecord {
                seasons: seasons(8),
                ..kb_series(802, "Glimmer", Some(2019))
            },
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let mut series: Vec<Option<u32>> = catalog.tv_series.iter().map(|s| s.tmdb_id).collect();
    series.sort();
    assert_eq!(series, vec![Some(702), Some(801)]);
}

#[tokio::test]
async fn a_lone_episode_named_with_its_number_is_found_as_one_of_its_series() {
    let files = [
        video(1, "[Fnord] Samurai Sentai Blorp 22 HD [ABCD1234].mkv", 24.0),
        // A movie whose title ends in a number is found as the movie.
        video(2, "Movies/Zibble 13 1080p WEB-DL.mkv", 100.0),
    ];
    let kb = CatalogKb {
        series: vec![
            kb_series(981, "Samurai Sentai Blorp", Some(2009)),
            kb_series(983, "Zibble", Some(2001)),
        ],
        movies: vec![kb_movie(982, "Zibble 13", Some(1995), None)],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "Samurai Sentai Blorp".to_string(),
            vec![(1, None, [22, 22])]
        )]
    );
    let movies: Vec<Option<u32>> = movies_by_id(&catalog)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(movies, vec![Some(982)]);
}

#[tokio::test]
async fn a_series_named_with_its_season_is_found_as_that_season() {
    let files: Vec<_> = (1..=3)
        .map(|n| {
            video(
                n,
                &format!(
                    "[Fnord] Zibble Lark 2nd Season/[Fnord] Zibble Lark 2nd Season - 0{n}.mkv"
                ),
                24.0,
            )
        })
        .collect();
    let kb = CatalogKb {
        series: vec![kb_series(991, "Zibble Lark", Some(2019))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "Zibble Lark".to_string(),
            vec![
                (1, Some(2), [1, 1]),
                (2, Some(2), [2, 2]),
                (3, Some(2), [3, 3])
            ]
        )]
    );
}

#[tokio::test]
async fn a_batchs_extras_named_as_specials_are_searched_for_by_their_own_names() {
    let release = "Zibble Saga Season 1 + Extras";
    // Not `road home`, named too like `zibble road` and `home run` to be one.
    let mut files: Vec<_> = (1..=16)
        .filter(|&n| n != 8)
        .map(|n| {
            let name = SIXTEEN_NAMES[n - 1];
            video(
                n as i64,
                &format!("{release}/Extras/Zibble Saga Extra - {name}.avi"),
                11.0,
            )
        })
        .collect();
    files.extend((1..=3).map(|n| {
        video(
            100 + n,
            &format!("{release}/Season 1/Zibble.Saga.S01E0{n}.avi"),
            25.0,
        )
    }));
    let specials = Season {
        number: 0,
        episodes: (1..=16)
            .map(|n| Episode {
                number: n,
                name: SIXTEEN_NAMES[n as usize - 1].to_string(),
                runtime_minutes: Some(11),
                production_code: None,
                aired: None,
            })
            .collect(),
    };
    let kb = CatalogKb {
        series: vec![kb_series(1101, "Zibble Saga", Some(2000))],
        seasons: vec![(1101, specials)],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let specials: Vec<(i64, Option<u32>, [u32; 2])> = series_episodes(&catalog)
        .into_iter()
        .flat_map(|(_, episodes)| episodes)
        .filter(|&(id, _, _)| id <= 16)
        .collect();
    // Each its special, though the series is found without their names.
    assert_eq!(specials.len(), 15, "{specials:?}");
    assert!(
        specials
            .iter()
            .all(|&(id, season, [first, _])| season == Some(0) && first == id as u32)
    );
}

#[tokio::test]
async fn a_season_release_also_known_as_another_name_is_that_ones_series() {
    let release = "Wumbo Lark AKA La Storia di Wumbo S02 1080p WEB-DL";
    let files: Vec<_> = (1..=3)
        .map(|n| {
            video(
                n,
                &format!("{release}/Wumbo.Lark.S02E0{n}.1080p.WEB-DL.mkv"),
                45.0,
            )
        })
        .collect();
    let kb = CatalogKb {
        series: vec![
            // Listed first, named the same.
            kb_series(1201, "Wumbo Lark", Some(2000)),
            SeriesRecord {
                original_name: "La Storia di Wumbo".into(),
                ..kb_series(1202, "Wumbo Lark", Some(2022))
            },
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let series: Vec<Option<u32>> = catalog.tv_series.iter().map(|s| s.tmdb_id).collect();
    assert_eq!(series, vec![Some(1202)]);
}

#[tokio::test]
async fn a_lone_episode_named_with_its_number_and_title_is_found_as_one_of_its_series() {
    let files = [
        video(1, "Glimmer.CX.260.Majuuou.720p.HDTV.x264-Blorp.mkv", 46.0),
        video(2, "[Fnord] Zibble Deka S1 _ 13 [ABCD1234].mkv", 24.0),
    ];
    let kb = CatalogKb {
        series: vec![
            kb_series(1301, "Glimmer CX", Some(2003)),
            kb_series(1302, "Zibble Deka", Some(1985)),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_episodes(&catalog),
        vec![
            ("Glimmer CX".to_string(), vec![(1, None, [260, 260])]),
            ("Zibble Deka".to_string(), vec![(2, Some(1), [13, 13])]),
        ]
    );
}

#[test]
fn a_videos_kind_of_work_is_claimed_by_its_name_and_release() {
    let files = [
        video(1, "Zibble.Lark.S01E02.1080p.WEB-DL.mkv", 45.0),
        video(2, "Wumbo.Night.2019.1080p.BluRay.x264-Blorp.mkv", 101.0),
        // Neither numbered nor dated: no kind.
        video(3, "Glimmer Peaks.mkv", 60.0),
    ];
    let claims = run_heuristics(&files);
    let kind = |subject: usize| {
        claims[&subject].iter().find_map(|claim| match claim.pred {
            ClaimPredicate::WorkKind(kind) => Some(kind),
            _ => None,
        })
    };
    assert_eq!(kind(0), Some(WorkKind::TvSeries));
    assert_eq!(kind(1), Some(WorkKind::Movie));
    assert_eq!(kind(2), None);
}

#[test]
fn a_video_with_no_kind_is_a_series_by_the_videos_near_it() {
    let files = [
        video(1, "Zibble Lark S01/Zibble.Lark.S01E01.mkv", 45.0),
        video(2, "Zibble Lark S01/Zibble.Lark.S01E02.mkv", 45.0),
        // Beside the release's episodes, a level down: a series'.
        video(3, "Zibble Lark S01/Featurettes/Making Of.mkv", 20.0),
        // Or two levels down.
        video(10, "Zibble Lark S01/Featurettes/Deleted/Scene.mkv", 3.0),
        // Beside a movie and a series' episode alike: no kind.
        video(
            4,
            "Glimmer/Wumbo.Night.2019.1080p.BluRay.x264-Blorp.mkv",
            101.0,
        ),
        video(5, "Glimmer/Yonder.S02E03.mkv", 45.0),
        video(6, "Glimmer/Fnord.mkv", 60.0),
        // Beside a movie: no kind, as it may well be another work's.
        video(
            7,
            "Wumbo Night (2019)/Wumbo.Night.2019.1080p.BluRay.mkv",
            101.0,
        ),
        video(8, "Wumbo Night (2019)/Trailer.mkv", 2.0),
        // At the library's root, beside every release: no kind.
        video(9, "Blorp.mkv", 30.0),
    ];
    let claims = run_heuristics(&files);
    let kind = |subject: usize| {
        claims[&subject].iter().find_map(|claim| match claim.pred {
            ClaimPredicate::WorkKind(kind) => Some(kind),
            _ => None,
        })
    };
    assert_eq!(kind(2), Some(WorkKind::TvSeries));
    assert_eq!(kind(3), Some(WorkKind::TvSeries));
    assert_eq!(kind(6), None);
    assert_eq!(kind(8), None);
    assert_eq!(kind(9), None);
}

#[test]
fn a_video_with_no_kind_is_a_movie_if_it_runs_as_long_as_one() {
    let files = [
        video(1, "Glimmer Peaks/Zibble.mkv", 95.0),
        // Short: as likely a movie's extra as an episode.
        video(2, "Glimmer Peaks/Wumbo.mkv", 24.0),
        // Numbered as an episode: a series', however long.
        video(3, "Yonder/Yonder.S01E01.mkv", 95.0),
    ];
    let claims = run_heuristics(&files);
    let kind = |subject: usize| {
        claims[&subject].iter().find_map(|claim| match claim.pred {
            ClaimPredicate::WorkKind(kind) => Some(kind),
            _ => None,
        })
    };
    assert_eq!(kind(0), Some(WorkKind::Movie));
    assert_eq!(kind(1), None);
    assert_eq!(kind(2), Some(WorkKind::TvSeries));
}

#[tokio::test]
async fn a_series_named_with_a_release_s_words_is_found_by_its_name_alone() {
    let files: Vec<_> = (1..=3)
        .map(|n| {
            video(
                n,
                &format!(
                    "[Fnord] Zibble Lark Remastered/[Fnord] Zibble Lark Remastered - 0{n}.mkv"
                ),
                24.0,
            )
        })
        .collect();
    let kb = CatalogKb {
        series: vec![kb_series(901, "Zibble Lark", Some(1996))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let series: Vec<(Option<u32>, usize)> = catalog
        .tv_series
        .iter()
        .map(|series| (series.tmdb_id, series.file_ids().count()))
        .collect();
    assert_eq!(series, vec![(Some(901), 3)]);
}

#[test]
fn a_series_name_loses_the_release_s_words_from_the_first_that_is_surely_one() {
    let files = [video(
        1,
        "Zibble Lark S01-S03 COMPLETE 480p XViD MP3 2.0/S01E01 Fnord vs Wumbo.avi",
        45.0,
    )];
    let claims = run_heuristics(&files);
    let names: Vec<&str> = claims[&0]
        .iter()
        .filter_map(|claim| match &claim.pred {
            ClaimPredicate::TvSeriesName(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(names.contains(&"zibble lark"), "{names:?}");
}

#[tokio::test]
async fn a_run_named_as_a_later_season_is_numbered_in_it_not_through_the_series() {
    let release = "[Fnord] Zibble Lark collection";
    let mut files: Vec<_> = (1..=3)
        .map(|n| {
            video(
                n,
                &format!("{release}/[Fnord] Zibble Lark/[Fnord] Zibble Lark 0{n}.mkv"),
                24.0,
            )
        })
        .collect();
    files.extend((1..=4).map(|n| {
        video(
            10 + n,
            &format!(
                "{release}/[Fnord] Zibble Lark ~Wumbo Nights~/[Fnord] Zibble Lark ~Wumbo Nights~ 0{n}.mkv"
            ),
            24.0,
        )
    }));
    let season = |number: u32, count: u32| Season {
        number,
        episodes: (1..=count)
            .map(|n| Episode {
                number: n,
                name: format!("Chapter {n}"),
                runtime_minutes: Some(24),
                production_code: None,
                aired: None,
            })
            .collect(),
    };
    let kb = CatalogKb {
        series: vec![kb_series(991, "Zibble Lark", Some(2001))],
        seasons: vec![(991, season(1, 3)), (991, season(2, 4))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "Zibble Lark".to_string(),
            vec![
                (1, Some(1), [1, 1]),
                (2, Some(1), [2, 2]),
                (3, Some(1), [3, 3]),
                (11, Some(2), [1, 1]),
                (12, Some(2), [2, 2]),
                (13, Some(2), [3, 3]),
                (14, Some(2), [4, 4]),
            ]
        )]
    );
}

#[tokio::test]
async fn a_runs_episodes_past_its_season_are_the_specials_that_aired_between() {
    let release = "[Fnord] Zibble Lark collection";
    let mut files: Vec<_> = (1..=5)
        .map(|n| {
            video(
                n,
                &format!("{release}/[Fnord] Zibble Lark/[Fnord] Zibble Lark 0{n}.mkv"),
                24.0,
            )
        })
        .collect();
    files.extend((1..=3).map(|n| {
        video(
            10 + n,
            &format!(
                "{release}/[Fnord] Zibble Lark ~Yonder Story~/[Fnord] Zibble Lark ~Yonder Story~ 0{n}.mkv"
            ),
            24.0,
        )
    }));
    let day = |month, day| {
        Some(Date {
            year: 2008,
            month,
            day,
        })
    };
    let episode = |number: u32, aired| Episode {
        number,
        name: format!("Chapter {number}"),
        runtime_minutes: Some(24),
        production_code: None,
        aired,
    };
    let kb = CatalogKb {
        series: vec![kb_series(991, "Zibble Lark", Some(2007))],
        seasons: vec![
            (
                991,
                Season {
                    number: 1,
                    episodes: (1..=3).map(|n| episode(n, day(1, n))).collect(),
                },
            ),
            (
                991,
                Season {
                    number: 2,
                    episodes: (1..=3).map(|n| episode(n, day(10, n))).collect(),
                },
            ),
            // Two aired between the seasons, one after.
            (
                991,
                Season {
                    number: 0,
                    episodes: vec![
                        episode(1, day(7, 1)),
                        episode(2, day(3, 1)),
                        episode(3, day(12, 1)),
                    ],
                },
            ),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "Zibble Lark".to_string(),
            vec![
                (1, Some(1), [1, 1]),
                (2, Some(1), [2, 2]),
                (3, Some(1), [3, 3]),
                // In the order they aired.
                (4, Some(0), [2, 2]),
                (5, Some(0), [1, 1]),
                (11, Some(2), [1, 1]),
                (12, Some(2), [2, 2]),
                (13, Some(2), [3, 3]),
            ]
        )]
    );
}

#[tokio::test]
async fn a_runs_episodes_past_its_season_are_no_later_runs_episodes() {
    let release = "[Fnord] Zibble Lark collection";
    let mut files: Vec<_> = (1..=5)
        .map(|n| {
            video(
                n,
                &format!("{release}/[Fnord] Zibble Lark/[Fnord] Zibble Lark 0{n}.mkv"),
                24.0,
            )
        })
        .collect();
    files.extend((1..=3).map(|n| {
        video(
            10 + n,
            &format!(
                "{release}/[Fnord] Zibble Lark ~Yonder Story~/[Fnord] Zibble Lark ~Yonder Story~ 0{n}.mkv"
            ),
            24.0,
        )
    }));
    let day = |month, day| {
        Some(Date {
            year: 2008,
            month,
            day,
        })
    };
    let episode = |number: u32, aired| Episode {
        number,
        name: format!("Chapter {number}"),
        runtime_minutes: Some(24),
        production_code: None,
        aired,
    };
    let kb = CatalogKb {
        series: vec![kb_series(991, "Zibble Lark", Some(2007))],
        seasons: vec![
            (
                991,
                Season {
                    number: 1,
                    episodes: (1..=3).map(|n| episode(n, day(1, n))).collect(),
                },
            ),
            (
                991,
                Season {
                    number: 2,
                    episodes: (1..=3).map(|n| episode(n, day(10, n))).collect(),
                },
            ),
            // One aired between the seasons, for two past the first: unknown.
            (
                991,
                Season {
                    number: 0,
                    episodes: vec![episode(1, day(7, 1)), episode(3, day(12, 1))],
                },
            ),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "Zibble Lark".to_string(),
            vec![
                (1, Some(1), [1, 1]),
                (2, Some(1), [2, 2]),
                (3, Some(1), [3, 3]),
                // Of no season, rather than the later run's first two.
                (4, None, [4, 4]),
                (5, None, [5, 5]),
                (11, Some(2), [1, 1]),
                (12, Some(2), [2, 2]),
                (13, Some(2), [3, 3]),
            ]
        )]
    );
}

#[tokio::test]
async fn a_season_folder_of_a_series_named_with_a_subtitle_stays_that_series() {
    let release = "[Fnord] Zibble Lark! (Wumbo no Glimmer) S1 - S2 [Full Series][BD]";
    let files: Vec<_> = (1..=4)
        .map(|n| {
            video(
                n,
                &format!("{release}/Zibble Lark! Season 2 [Dual Audio]/Zibble Lark - 0{n}.mkv"),
                24.0,
            )
        })
        .collect();
    let seasons = |counts: &[u32]| -> Vec<SeasonSummary> {
        counts
            .iter()
            .enumerate()
            .map(|(i, &episode_count)| SeasonSummary {
                number: i as u32 + 1,
                name: format!("Season {}", i + 1),
                episode_count,
            })
            .collect()
    };
    let kb = CatalogKb {
        series: vec![
            SeriesRecord {
                seasons: seasons(&[24, 13]),
                ..kb_series(601, "Zibble Lark! Wumbo no Glimmer", Some(2015))
            },
            // Named as the files are, one with no second season, one with a
            // second as long as the release's.
            SeriesRecord {
                seasons: seasons(&[8]),
                ..kb_series(602, "Zibble Lark", Some(2026))
            },
            SeriesRecord {
                seasons: seasons(&[20, 4]),
                ..kb_series(603, "Zibble Lark", Some(2010))
            },
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let series: Vec<(Option<u32>, usize)> = catalog
        .tv_series
        .iter()
        .map(|s| (s.tmdb_id, s.file_ids().count()))
        .collect();
    assert_eq!(series, vec![(Some(601), 4)]);
}

#[tokio::test]
async fn an_ova_one_of_whose_titles_finds_a_movie_is_that_movie_not_a_special() {
    let release = "[Fnord] Zibble Lark (Seasons 1-2 + OVAs)/[Fnord] Zibble Lark OVAs";
    let files = [
        video(
            1,
            &format!("{release}/[Fnord] Zibble Lark OVA01 - Wumbo Nights.mkv"),
            28.0,
        ),
        video(
            2,
            &format!("{release}/[Fnord] Zibble Lark OVA02 - Glimmer Days.mkv"),
            28.0,
        ),
        video(
            3,
            &format!("{release}/[Fnord] Zibble Lark OVA03 - Yonder Fair.mkv"),
            28.0,
        ),
    ];
    let specials = Season {
        number: 0,
        episodes: ["Wumbo Nights", "Glimmer Days", "Yonder Fair"]
            .iter()
            .enumerate()
            .map(|(i, name)| Episode {
                number: i as u32 + 1,
                name: name.to_string(),
                runtime_minutes: Some(28),
                production_code: None,
                aired: None,
            })
            .collect(),
    };
    let kb = CatalogKb {
        series: vec![kb_series(701, "Zibble Lark", Some(2006))],
        seasons: vec![(701, specials)],
        // Each OVA is a movie too, found by its title without its number.
        movies: vec![
            kb_movie(801, "Zibble Lark: Wumbo Nights", Some(2008), None),
            kb_movie(802, "Zibble Lark: Glimmer Days", Some(2009), None),
            kb_movie(803, "Zibble Lark: Yonder Fair", Some(2012), None),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(Some(801), 1), (Some(802), 2), (Some(803), 3)]);
}
