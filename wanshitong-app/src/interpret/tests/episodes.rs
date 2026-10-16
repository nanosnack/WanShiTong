//! Episodes: their seasons, numbers and titles.

use super::*;

#[test]
fn an_episode_is_titled_after_its_marker() {
    let files = [
        video(1, "Alfred Hitchcock Presents - S02E07 - Alibi Me.avi", 25.0),
        video(
            2,
            "Hunter x Hunter (2011) - S02E61 (119) - Strong x Or x Weak.mkv",
            24.0,
        ),
        video(
            3,
            "Avatar.The.Last.Airbender.S02E14.City.of.Walls.and.Secrets.1080p.BluRay.REMUX.mkv",
            24.0,
        ),
        video(
            4,
            "SpongeBob.SquarePants.S01E34.Arrgh.REPACK2.1080p.AMZN.WEB-DL.mkv",
            11.0,
        ),
        video(5, "Dragnet-S04E15-Homicide-Who.Killed.Who.mkv", 25.0),
        // `Internal` in a title, but `AMZN` a release's.
        video(9, "Dragnet-S03E12-Internal.Affairs-DR-20.mkv", 25.0),
        video(
            10,
            "SpongeBob.SquarePants.S03E28.Mid-Life.Crustacean.AMZN.WEB-DL.DDP2.0.mkv",
            11.0,
        ),
        // No title, or only the episode's number again.
        video(6, "Breaking.Bad.S03E03.1080p.BluRay.REMUX.mkv", 47.0),
        video(7, "Babylon Berlin S01E09 Episode 9.mkv", 45.0),
        // Two episodes, whose titles its name joins.
        video(
            8,
            "Avatar.The.Last.Airbender.S02E12E13.The.Serpents.Pass.and.The.Drill.mkv",
            48.0,
        ),
    ];
    let claims = run_heuristics(&files);
    let files = &files;

    let titles: Vec<(i64, &str)> = (0..files.len())
        .flat_map(|subject| {
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter_map(move |claim| match &claim.pred {
                    ClaimPredicate::EpisodeTitle(title) => {
                        Some((files[subject].1.row_id.0, title.as_str()))
                    }
                    _ => None,
                })
        })
        .collect();
    assert_eq!(
        titles,
        vec![
            (1, "Alibi Me"),
            (2, "Strong x Or x Weak"),
            (3, "City of Walls and Secrets"),
            (4, "Arrgh"),
            (5, "Homicide-Who Killed Who"),
            (9, "Internal Affairs-DR-20"),
            (10, "Mid-Life Crustacean"),
        ]
    );
}

#[test]
fn a_season_directory_says_a_number_is_its_season_and_an_episode() {
    let files = [
        video(
            1,
            "Perry Mason/Season 1/Perry.Mason.101.The.Case.of.the.Restless.Redhead.(DVDRip).avi",
            53.0,
        ),
        video(
            2,
            "Perry Mason/Season 1/Perry.Mason.102.The.Case.of.the.Sleepwalker's.Niece.(DVDRip).avi",
            53.0,
        ),
        video(
            3,
            "Perry Mason/Season 7/Perry.Mason.730.The.Ugly.Duckling.(TVRip).avi",
            51.0,
        ),
        // Not the season its directory names.
        video(
            4,
            "Perry Mason/Season 2/Perry.Mason.101.The.Pilot.avi",
            53.0,
        ),
        // No season directory to say what the number is.
        video(
            5,
            "Movies/Perry.Mason.101.The.Case.of.the.Lost.Reel.avi",
            53.0,
        ),
        // Not a number of its own.
        video(6, "Movies/Season 7 Films/Some.Film.720p.mkv", 90.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "perry mason".into(),
            vec![
                (1, Some(1), [1, 1]),
                (2, Some(1), [2, 2]),
                (3, Some(7), [30, 30])
            ]
        )]
    );
}

#[tokio::test]
async fn a_season_directory_s_number_competes_with_an_episode_marker() {
    let files = [
        video(
            1,
            "Perry Mason/Season 5/Perry.Mason.514.The.Case.of.the.Unwelcome.Bride.avi",
            51.0,
        ),
        // `E138` is numbered through the whole series, and the name before
        // it isn't the series'.
        video(
            2,
            "Perry Mason/Season 5/Perry.Mason.515.The.Case.of.the.Roving.River.(E138).avi",
            51.0,
        ),
    ];
    let kb = series_kb(vec![(4177, "Perry Mason", 1957)]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "Perry Mason".into(),
            vec![(1, Some(5), [14, 14]), (2, Some(5), [15, 15])]
        )]
    );
}

#[test]
fn a_codec_s_number_and_a_marked_episode_s_aren_t_a_season_directory_s() {
    let king = "King of the Hill/King.of.the.Hill.S02.1080p.DSNP.WEBRip.AAC2.0.x264-ZgR";
    let files = [
        // `264` in `H.264` isn't season 2, episode 64.
        video(
            1,
            &format!(
                "{king}/King.of.the.Hill.S02E06.Husky.Bobby.1080p.DSNP.WEB-DL.AAC2.0.H.264-ZgR.mkv"
            ),
            22.0,
        ),
        video(
            2,
            "King of the Hill/Season 2/King.of.the.Hill.Husky.Bobby.1080p.H.264.mkv",
            22.0,
        ),
        // `S01E05` says the episode outright, so `101` is the title's.
        video(
            3,
            "Perry Mason/Season 1/Perry.Mason.S01E05.The.Case.of.Room.101.avi",
            51.0,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![
            ("king of the hill".into(), vec![(1, Some(2), [6, 6])]),
            ("perry mason".into(), vec![(3, Some(1), [5, 5])]),
        ]
    );
    for (file_id, log) in &catalog.audit_logs {
        assert!(
            !log.claims.iter().any(|claim| claim.source
                == ClaimSource::Heuristic(Heuristic::SeasonEpisodeNumber)),
            "file {} has a season directory number claim",
            file_id.0
        );
    }
}

#[test]
fn a_file_can_be_several_episodes_or_a_version_of_one() {
    let avatar = "Avatar the Last Airbender (2005)/Avatar.The.Last.Airbender.S03.1080p.BluRay.REMUX.AVC.FLAC.2.0-NOGRP[splonk]";
    let dan_da_dan =
        "DAN.DA.DAN/DAN DA DAN (2024) S02 (1080p CR WEB-DL H264 AAC 2.0 Dual-Audio) [Wubble]";
    let files = [
        video(
            1,
            &format!(
                "{avatar}/Avatar.The.Last.Airbender.S03E14E15.The.Boiling.Rock.1080p.BluRay.REMUX.AVC.FLAC.2.0-NOGRP.mkv"
            ),
            46.1,
        ),
        video(
            2,
            &format!(
                "{avatar}/Avatar.The.Last.Airbender.S03E18E19E20E21.Sozins.Comet.1080p.BluRay.REMUX.AVC.FLAC.2.0-Thud.mkv"
            ),
            91.3,
        ),
        video(3, "Adventure Time/Adventure.Time.S07E14-E15.mkv", 22.6),
        video(4, "Adventure Time/Adventure.Time.S07E16-17.mkv", 22.6),
        video(
            5,
            &format!(
                "{dan_da_dan}/DAN DA DAN (2024) S02E01v2 (1080p CR WEB-DL H264 AAC 2.0 Dual-Audio) [Wubble].mkv"
            ),
            24.2,
        ),
        // A resolution, not the last episode.
        video(6, "The Wire/The.Wire.S01E05-1080p.mkv", 58.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![
            (
                "adventure time".into(),
                vec![(3, Some(7), [14, 15]), (4, Some(7), [16, 17])]
            ),
            (
                "avatar the last airbender".into(),
                vec![(1, Some(3), [14, 15]), (2, Some(3), [18, 21])]
            ),
            ("dan da dan".into(), vec![(5, Some(2), [1, 1])]),
            ("the wire".into(), vec![(6, Some(1), [5, 5])]),
        ]
    );
    assert_eq!(movies(&catalog), vec![]);
}

#[test]
fn a_roman_numeral_after_a_series_name_is_its_season() {
    let mob = "Mob.Psycho/Mob Psycho 100";
    let s1 = "Mob Psycho 100 [2016] COMPLETE 1080p BDRip x265 PCM 2.0 Blarg [Flob]";
    let s3 = "Mob Psycho 100 III [2022] COMPLETE 1080p BDRip x265 PCM 2.0 Blarg [Flob]";
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
        video(
            4,
            &format!(
                "{mob}/{s3}/Mob Psycho 100 III - 01 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv"
            ),
            23.7,
        ),
        video(
            5,
            &format!(
                "{mob}/{s3}/Mob Psycho 100 III - 02 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv"
            ),
            23.7,
        ),
        video(
            6,
            &format!(
                "{mob}/{s3}/Mob Psycho 100 III - 03 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv"
            ),
            23.7,
        ),
        // Before an episode marker.
        video(7, "Overlord/Overlord II Episode 05.mkv", 24.0),
        // Part of the name, not a season.
        video(8, "Gundam X/Mobile Suit Gundam X - 01.mkv", 24.0),
        video(9, "Gundam X/Mobile Suit Gundam X - 02.mkv", 24.0),
        video(10, "Gundam X/Mobile Suit Gundam X - 03.mkv", 24.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![
            (
                "mob psycho 100".into(),
                vec![
                    (1, None, [1, 1]),
                    (2, None, [2, 2]),
                    (3, None, [3, 3]),
                    (4, Some(3), [1, 1]),
                    (5, Some(3), [2, 2]),
                    (6, Some(3), [3, 3]),
                ]
            ),
            (
                "mobile suit gundam x".into(),
                vec![(8, None, [1, 1]), (9, None, [2, 2]), (10, None, [3, 3])]
            ),
            ("overlord".into(), vec![(7, Some(2), [5, 5])]),
        ]
    );
}

#[test]
fn a_season_just_before_an_episode_number_is_its_season() {
    let curb = "Curb Your Enthusiasm Season 1, 2, 3, 4, 5, 6, 7 & 8 + Extras DVDRip Zib";
    let files = [
        video(
            1,
            &format!(
                "{curb}/Season 1/Curb Your Enthusiasm Season 1 Episode 01 - The Pants Tent.avi"
            ),
            28.0,
        ),
        video(
            2,
            &format!(
                "{curb}/Season 8/Curb Your Enthusiasm Season 8 Episode 10 - Larry vs. Michael J. Fox.avi"
            ),
            28.0,
        ),
        // Of the series its episodes name.
        video(3, &format!("{curb}/Extras/Larry David Interview.avi"), 20.0),
        // Without a season directory.
        video(4, "The.Office.S02.E05.720p.mkv", 22.0),
        // Without a season.
        video(
            5,
            "[Wobble6464] Serial Experiments Lain (JPBD-Remux)(Dual-Audio)/Serial Experiments Lain - Episode 01 [Wobble6464].mkv",
            23.9,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![
            (
                "curb your enthusiasm".into(),
                vec![(1, Some(1), [1, 1]), (2, Some(8), [10, 10])]
            ),
            ("serial experiments lain".into(), vec![(5, None, [1, 1])]),
            ("the office".into(), vec![(4, Some(2), [5, 5])]),
        ]
    );
    let curb_extras: Vec<_> = catalog
        .tv_series
        .iter()
        .filter(|series| series.name == "curb your enthusiasm")
        .flat_map(|series| series.extras.iter().map(|id| id.0))
        .collect();
    assert_eq!(curb_extras, vec![3]);
}

#[test]
fn episodes_without_a_season_are_numbered_through_the_series() {
    let one_piece = "One Piece (1999) [tvdbid-81797]";
    let lain = "[Wobble6464] Serial Experiments Lain (JPBD-Remux)(Dual-Audio)";
    let food_wars = "[Fnord Bleep] Food Wars! (Shokugeki no Soma) S1 - S5 [Full Series][BD]";
    let files = [
        video(
            1,
            &format!("{one_piece}/Arc 12 - Arabasta/One.Piece.E92.1080p.mkv"),
            20.5,
        ),
        video(
            2,
            &format!("{one_piece}/Arc 12 - Arabasta/One.Piece.E93-E95.1080p.mkv"),
            28.4,
        ),
        video(
            3,
            &format!("{one_piece}/Arc 01 - Romance Dawn/One.Piece.S00E36.1080p.mkv"),
            18.3,
        ),
        // In a directory of its own, but not a movie's.
        video(
            4,
            &format!("{lain}/Serial Experiments Lain - Episode 01 [Wobble6464].mkv"),
            23.9,
        ),
        // Numbered within the season its directory names.
        video(5, "Doctor Who/Season 2/Doctor Who - Episode 03.mkv", 45.0),
        video(
            6,
            &format!("{food_wars}/Food Wars! Season 4 [Dual Audio]/Food Wars - Ep 01.mkv"),
            23.7,
        ),
        // Several seasons are no season.
        video(7, &format!("{food_wars}/Food Wars - Episode 07.mkv"), 23.7),
        video(8, "Code Geass (Seasons 1-2)/Code Geass - E07.mkv", 24.0),
        // No episode here.
        video(
            9,
            "Heat (1995)/Heat.1995.2160p.BluRay.E-AC3.HEVC.mkv",
            170.0,
        ),
        // Episode 13, alone in its directory, and its CRC not episode
        // 2244536.
        video(
            10,
            "(frobnitz-b'ork) Dr. Slump & Arale-chan (Ep. 1-54)/Dr. Slump & Arale-chan 013 V2 [D30B6F1A] [b'ork].mkv",
            24.7,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![
            ("code geass".into(), vec![(8, None, [7, 7])]),
            ("doctor who".into(), vec![(5, Some(2), [3, 3])]),
            ("dr slump & arale-chan".into(), vec![(10, None, [13, 13])]),
            (
                "food wars".into(),
                vec![(6, Some(4), [1, 1]), (7, None, [7, 7])]
            ),
            (
                "one piece".into(),
                vec![
                    (1, None, [92, 92]),
                    (2, None, [93, 95]),
                    (3, Some(0), [36, 36])
                ]
            ),
            ("serial experiments lain".into(), vec![(4, None, [1, 1])]),
        ]
    );
    assert!(movies(&catalog).contains(&("heat".into(), vec![(9, Some(FileRole::Main))])));
}

/// A knowledge base of Dragnet, with its episodes of a list of (title,
/// season, episode), which a file can be found to be by its title.
fn titles_kb(titled: Vec<(&str, u32, u32)>) -> CatalogKb {
    coded_kb(
        titled
            .into_iter()
            .map(|(name, season, episode)| (name, None, season, episode))
            .collect(),
    )
}

/// A knowledge base of Dragnet, with its episodes of a list of (title,
/// production code, season, episode).
fn coded_kb(titled: Vec<(&str, Option<&str>, u32, u32)>) -> CatalogKb {
    let mut seasons: Vec<(u32, Season)> = Vec::new();
    for (name, code, season, episode) in titled {
        let episode = Episode {
            number: episode,
            name: name.into(),
            runtime_minutes: None,
            production_code: code.map(String::from),
            aired: None,
        };
        match seasons.iter_mut().find(|(_, s)| s.number == season) {
            Some((_, s)) => s.episodes.push(episode),
            None => seasons.push((
                45194,
                Season {
                    number: season,
                    episodes: vec![episode],
                },
            )),
        }
    }
    CatalogKb {
        seasons,
        ..series_kb(vec![(45194, "Dragnet", 1967)])
    }
}

/// Dragnet's release is numbered in another order than TMDB's. Two files
/// are renumbered by their titles, one onto the episode a third's name
/// numbers it as, whose title TMDB doesn't know it by. Both are kept as
/// manifestations of that episode: nothing says which is wrong, and the
/// files aren't settled against each other.
#[tokio::test]
async fn files_numbered_as_one_episode_are_its_manifestations() {
    let release = "Dragnet.(1967).Complete/Season 3";
    let other = "Dragnet 1967 S03 DVDRip";
    let files = [
        video(
            1,
            &format!("{release}/Dragnet-S03E19-Juvenile.Division-DR-19.mkv"),
            25.0,
        ),
        video(
            2,
            &format!("{release}/Dragnet-S03E20-Burglary-DR-31.mkv"),
            25.0,
        ),
        video(
            3,
            &format!("{release}/Dragnet-S03E21-Frauds-DR-28.mkv"),
            25.0,
        ),
        video(4, &format!("{release}/Dragnet-S03E22-Vice-DR-30.mkv"), 25.0),
        // Another release's manifestation of the episode.
        video(5, &format!("{other}/Dragnet.S03E19.mkv"), 25.0),
    ];
    let kb = titles_kb(vec![("Burglary-DR-31", 3, 21), ("Frauds-DR-28", 3, 19)]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![
            (1, Some(3), [19, 19]),
            (2, Some(3), [21, 21]),
            (3, Some(3), [19, 19]),
            (4, Some(3), [22, 22]),
            (5, Some(3), [19, 19]),
        ]
    );
    assert!(series.extras.is_empty());
}

/// Files keep their whole ranges, even where they overlap, as One Piece's
/// do, and even where one begins on another's first episode: which of them
/// a view links is the view's choice.
#[tokio::test]
async fn overlapping_files_keep_their_whole_ranges() {
    let release = "Dragnet.(1967).Complete/Season 3";
    let files = [
        video(1, &format!("{release}/Dragnet.S03E07-E10.mkv"), 100.0),
        video(2, &format!("{release}/Dragnet.S03E10-E11.mkv"), 50.0),
        // Begins on the same episode as a file of more episodes.
        video(3, &format!("{release}/Dragnet.S03E12.mkv"), 25.0),
        video(4, &format!("{release}/Joined/Dragnet.S03E12-E13.mkv"), 50.0),
    ];
    let catalog = catalog_with_kb(&files, &[], &titles_kb(vec![])).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![
            (1, Some(3), [7, 10]),
            (2, Some(3), [10, 11]),
            (3, Some(3), [12, 12]),
            (4, Some(3), [12, 13]),
        ]
    );
    assert!(series.extras.is_empty());
}

/// An assertion about one file numbers that file alone: another file that a
/// title numbers as the same episode keeps that numbering, as a second
/// manifestation of the episode.
#[tokio::test]
async fn an_assertion_about_one_file_doesnt_renumber_another() {
    let media = DataSourceUuid(Uuid::nil());
    let release = "Dragnet.(1967).Complete/Season 3";
    let asserted = format!("{release}/Dragnet-S03E19-Juvenile.Division-DR-19.mkv");
    let files = [
        video(1, &asserted, 25.0),
        video(
            2,
            &format!("{release}/Dragnet-S03E21-Frauds-DR-28.mkv"),
            25.0,
        ),
    ];
    // Asserted, it outranks the other's title.
    let assertions = [
        assertion(media, &[&asserted], ClaimPredicate::Season(3)),
        assertion(media, &[&asserted], ClaimPredicate::EpisodeRange([19, 19])),
    ];
    let kb = titles_kb(vec![("Frauds-DR-28", 3, 19)]);
    let catalog = catalog_with_kb(&files, &assertions, &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![(1, Some(3), [19, 19]), (2, Some(3), [19, 19])]
    );
    assert!(series.extras.is_empty());
}

/// A knowledge base's result that renumbers a file by its title loses its
/// numbers to asserted ones, but not its series or the title.
#[tokio::test]
async fn asserted_numbers_outrank_a_titles_but_the_series_stays() {
    let media = DataSourceUuid(Uuid::nil());
    let path = "Alfred Hitchcock Presents - S01E30 - Alibi Me.avi";
    let files = [video(1, path, 25.0)];
    let assertions = [
        assertion(media, &[path], ClaimPredicate::Season(1)),
        assertion(media, &[path], ClaimPredicate::EpisodeRange([30, 30])),
    ];
    let claim = |pred| Ranked::new(80.0, pred);
    let kb = FixedKb(vec![vec![
        claim(ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(5273))),
        claim(ClaimPredicate::TvSeriesName(
            "Alfred Hitchcock Presents".into(),
        )),
        claim(ClaimPredicate::EpisodeTitle("Alibi Me".into())),
        claim(ClaimPredicate::Season(2)),
        claim(ClaimPredicate::EpisodeRange([7, 7])),
    ]]);
    let catalog = catalog_with_kb(&files, &assertions, &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    assert_eq!(series.tmdb_id, Some(5273));
    let episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.season_number, e.episode_range, e.episode_title.as_deref()))
        .collect();
    assert_eq!(episodes, vec![(Some(1), [30, 30], Some("Alibi Me"))]);
}

/// A release that names its files by the studio's production codes, as
/// Dragnet's does with `DR-19`, is numbered by them when TMDB knows the
/// codes, whatever the episodes are called. A code of digits alone is too
/// like a number in a title to go by.
#[tokio::test]
async fn an_episode_is_found_by_its_production_code() {
    let release = "Dragnet.(1967).Complete/Season 3";
    let files = [
        video(
            1,
            &format!("{release}/Dragnet-S03E19-Juvenile.Division-DR-19.mkv"),
            25.0,
        ),
        video(2, &format!("{release}/Dragnet-S03E20-Vice-DR-30.mkv"), 25.0),
        // Its code is a number, and the title has it.
        video(3, &format!("{release}/Dragnet-S03E21-Frauds-28.mkv"), 25.0),
    ];
    let kb = coded_kb(vec![
        // TMDB calls the episode something else, but knows its code.
        ("The Big Kid", Some("DR-19"), 3, 20),
        ("Vice: DR-30", Some("DR-30"), 3, 19),
        ("Frauds", Some("28"), 3, 25),
    ]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![
            (1, Some(3), [20, 20]),
            (2, Some(3), [19, 19]),
            (3, Some(3), [21, 21]),
        ]
    );
}

/// A file's title may say more than the episode's name, e.g. the studio's
/// code after it, or a longer form of the name. The episode whose name's
/// words the title has, in order, is the one, if it's the only such episode
/// and the name is more than one word.
#[tokio::test]
async fn an_episode_is_found_by_its_name_within_the_title() {
    let release = "Dragnet.(1967).Complete/Season 3";
    let files = [
        video(
            1,
            &format!("{release}/Dragnet-S03E19-Juvenile.Division-DR-19.mkv"),
            25.0,
        ),
        // `Vice` alone is too little to find an episode by.
        video(
            2,
            &format!("{release}/Dragnet-S03E20-Vice.Squad-DR-30.mkv"),
            25.0,
        ),
        // Two episodes' names are within it.
        video(
            3,
            &format!("{release}/Dragnet-S03E21-The.Big.Kid.Brother-DR-28.mkv"),
            25.0,
        ),
    ];
    let kb = titles_kb(vec![
        ("Juvenile: DR-19", 3, 22),
        ("Vice", 3, 23),
        ("The Big Kid", 3, 24),
        ("Big Brother", 3, 25),
    ]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![
            (1, Some(3), [22, 22]),
            (2, Some(3), [20, 20]),
            (3, Some(3), [21, 21]),
        ]
    );
}

/// A release may drop a word of an episode's name, e.g. SpongeBob's `Mermaid
/// and Barnacle Boy V` for TMDB's `Mermaid Man and Barnacle Boy V`. The
/// episode whose name has the title's words, in order, is the one, if it's
/// the only such episode: the numeral tells `V` from `IV` and `VI`.
#[tokio::test]
async fn an_episode_is_found_by_the_title_within_its_name() {
    let release = "SpongeBob.SquarePants/SpongeBob.SquarePants.S03.1080p";
    let files = [
        video(
            1,
            &format!("{release}/SpongeBob.SquarePants.S03E20.Mermaid.and.Barnacle.Boy.V.1080p.mkv"),
            11.0,
        ),
        // Within several names, so none.
        video(
            2,
            &format!("{release}/SpongeBob.SquarePants.S03E22.Mermaid.and.Barnacle.Boy.1080p.mkv"),
            11.0,
        ),
    ];
    let kb = CatalogKb {
        seasons: vec![(
            387,
            Season {
                number: 3,
                episodes: [
                    (19, "Mermaid Man and Barnacle Boy IV"),
                    (20, "Chocolate with Nuts"),
                    (21, "Mermaid Man and Barnacle Boy V"),
                    (23, "Mermaid Man and Barnacle Boy VI"),
                ]
                .into_iter()
                .map(|(number, name)| Episode {
                    number,
                    name: name.into(),
                    runtime_minutes: None,
                    production_code: None,
                    aired: None,
                })
                .collect(),
            },
        )],
        ..series_kb(vec![(387, "SpongeBob SquarePants", 1999)])
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![(1, Some(3), [21, 21]), (2, Some(3), [22, 22])]
    );
}

/// Anime releases number an episode through the whole series in
/// parentheses after its season's number, e.g. `S02E72 (130)`. Where the
/// release splits its seasons otherwise than TMDB, which numbers Hunter x
/// Hunter's season 2 from 63, the number through the series says which
/// episode the file is on TMDB, and a title TMDB knows still comes first.
#[tokio::test]
async fn a_number_through_the_series_in_parentheses_numbers_a_file_as_tmdb_does() {
    let hxh = "Hunter X Hunter (2011)/Season 2";
    let files = [
        video(
            1,
            &format!("{hxh}/Hunter x Hunter (2011) - S02E72 (130) - Magic x Of x Despair.mkv"),
            23.0,
        ),
        video(
            2,
            &format!("{hxh}/Hunter x Hunter (2011) - S02E73 (131) - Some x Title.mkv"),
            23.0,
        ),
        // Its title finds it, which outranks its numbers.
        video(
            3,
            &format!("{hxh}/Hunter x Hunter (2011) - S02E14 (072) - Chase x And x Chance.mkv"),
            23.0,
        ),
    ];
    let episodes = |numbers: std::ops::RangeInclusive<u32>, named: &[(u32, &str)]| {
        numbers
            .map(|number| Episode {
                number,
                name: named
                    .iter()
                    .find(|(n, _)| *n == number)
                    .map_or_else(|| format!("Episode {number}"), |(_, name)| name.to_string()),
                runtime_minutes: None,
                production_code: None,
                aired: None,
            })
            .collect::<Vec<_>>()
    };
    let kb = CatalogKb {
        seasons: vec![
            (
                46298,
                Season {
                    number: 1,
                    episodes: episodes(1..=62, &[]),
                },
            ),
            (
                46298,
                Season {
                    number: 2,
                    episodes: episodes(
                        63..=136,
                        &[(72, "Chase x And x Chance"), (130, "Magic × To × Destroy")],
                    ),
                },
            ),
        ],
        ..series_kb(vec![(46298, "Hunter x Hunter", 2011)])
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let [series] = &catalog.tv_series[..] else {
        panic!("one series");
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.season_number, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(
        episodes,
        vec![
            (1, Some(2), [130, 130]),
            (2, Some(2), [131, 131]),
            (3, Some(2), [72, 72]),
        ]
    );
}

/// A number in parentheses means many things, so it's a number through the
/// series only where the files beside it agree on how far it is past the
/// episode's number. Not a year, a download's `(2)`, or season 1, where the
/// two numbers are one.
#[test]
fn a_number_in_parentheses_is_through_the_series_only_when_its_siblings_agree() {
    let files = [
        // Agree: 58 past.
        video(1, "HxH/Season 2/HxH (2011) - S02E72 (130) - A.mkv", 23.0),
        video(2, "HxH/Season 2/HxH (2011) - S02E73 (131) - B.mkv", 23.0),
        // Alone with its offset.
        video(3, "HxH/Season 2/HxH (2011) - S02E74 (140) - C.mkv", 23.0),
        // A download's copy, and season 1.
        video(4, "Show/Season 2/Show - S02E01 (2).mkv", 23.0),
        video(5, "Show/Season 2/Show - S02E02 (3).mkv", 23.0),
        video(6, "Show/Season 1/Show - S01E01 (5).mkv", 23.0),
        video(7, "Show/Season 1/Show - S01E02 (6).mkv", 23.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let absolute = |file_id| {
        active_claims(&catalog, file_id)
            .into_iter()
            .find_map(|claim| match claim {
                ClaimPredicate::AbsoluteEpisodeRange([first, _]) => Some(first),
                _ => None,
            })
    };
    assert_eq!(absolute(1), Some(130));
    assert_eq!(absolute(2), Some(131));
    assert_eq!(absolute(3), None);
    // Files 4 and 5 agree on an offset of 1, which is all a guard on the
    // offset can tell; a knowledge base's seasons settle them.
    assert_eq!(absolute(4), Some(2));
    assert_eq!(absolute(6), None);
    assert_eq!(absolute(7), None);
}

#[test]
fn an_episode_is_marked_between_underscores_or_as_season_x_episode() {
    let files = [
        video(1, "Wumbo Quest S01/Wumbo Quest_S01E01_Pilot.mkv", 22.0),
        video(
            2,
            "Wumbo Quest S01/Wumbo Quest_S01E02 1080p WEB-DL.mkv",
            22.0,
        ),
        video(3, "Zibble S03/3x01 The Empty Lark.mkv", 88.0),
        video(4, "Zibble S03/Zibble 3x02 The Hollow Lark.mkv", 88.0),
        // A resolution's and a sound's numbers aren't episodes'.
        video(5, "Yonder (2001)/Yonder 640x480 DD5.1x264.mkv", 101.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        series_episodes(&catalog),
        vec![
            (
                "wumbo quest".into(),
                vec![(1, Some(1), [1, 1]), (2, Some(1), [2, 2])]
            ),
            (
                "zibble".into(),
                vec![(3, Some(3), [1, 1]), (4, Some(3), [2, 2])]
            ),
        ]
    );
    assert_eq!(
        movies(&catalog),
        vec![("yonder".into(), vec![(5, Some(FileRole::Main))])]
    );
}

#[test]
fn a_lone_video_numbered_apart_from_its_title_is_an_episode() {
    let files = [
        video(
            1,
            "[Blorp]_Wumbo_Quest_-_19_[h264-720p][07204822].mkv",
            24.0,
        ),
        video(2, "Zibble/Yonder Lark 02 [720p].mp4", 24.0),
        video(3, "[Blorp][Glimmer Peaks][03][BIG5][720P].mp4", 24.0),
        // Sequels and years, not episodes.
        video(4, "Movies/Toy Story 2.mkv", 92.0),
        video(5, "Concerts/Wumbo Live - 2019.mkv", 120.0),
        // A code, a size, and a movie's year.
        video(6, "Movies/ZK-19 - The Fnord (2002) 2160p.mkv", 138.0),
        video(7, "Movies/Glimmer 1080p x264 - 1.30GB.mp4", 101.0),
        video(8, "Movies/Yonder Street - 1666 (2021) 1080p.mkv", 114.0),
        video(9, "blorp-zibblequest-1080.mkv", 99.0),
        video(10, "Movies/ZQ-01.mkv", 96.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        series_episodes(&catalog),
        vec![
            ("glimmer peaks".into(), vec![(3, None, [3, 3])]),
            ("wumbo quest".into(), vec![(1, None, [19, 19])]),
            ("yonder lark".into(), vec![(2, None, [2, 2])]),
        ]
    );
}

#[test]
fn an_episode_is_marked_as_japanese_and_chinese_names_write_it() {
    let files = [
        video(
            1,
            "ウンボ[1983]/[アニメ DVD] ウンボ 第140話「題名のない話」 (640x480 x264).mkv",
            24.0,
        ),
        video(
            2,
            "ウンボ[1983]/[アニメ DVD] ウンボ 第141話「次の話」 (640x480 x264).mkv",
            24.0,
        ),
        video(3, "格林/格林 第12集.mp4", 45.0),
        video(4, "Glimmer/グリマー 第01-39話 Fin.ts", 900.0),
        // In a tag of its own, after the series' name in tags.
        video(5, "[字幕组][怪奇][第08话][GB][480P].mkv", 25.0),
        // An event's number, not an episode's.
        video(6, "第70回 ヨンダー歌合戦.ts", 270.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        series_episodes(&catalog),
        vec![
            (
                "ウンボ".into(),
                vec![(1, None, [140, 140]), (2, None, [141, 141])]
            ),
            ("グリマー".into(), vec![(4, None, [1, 39])]),
            ("怪奇".into(), vec![(5, None, [8, 8])]),
            ("格林".into(), vec![(3, None, [12, 12])]),
        ]
    );
}
