//! Runs of videos numbered alike, as a series' episodes or movies.

use super::*;

#[test]
fn videos_numbered_in_a_run_are_episodes() {
    // Three videos of a directory, `first_id` onwards, named by `name` for
    // each of their numbers.
    let run = |first_id: i64, numbers: [&str; 3], name: &dyn Fn(&str) -> String| {
        (first_id..)
            .zip(numbers)
            .map(|(id, number)| video(id, &name(number), 24.0))
            .collect::<Vec<_>>()
    };
    let chibi = "[Florp] Chibi Maruko-chan";
    let food_wars = "[Fnord Bleep] Food Wars! (Shokugeki no Soma) S1 - S5 [Full Series][BD][Dual Audio][HEVC 10bit x265][AAC][Eng Sub]";
    let mob = "Mob.Psycho/Mob Psycho 100/Mob Psycho 100 [2016] COMPLETE 1080p BDRip x265 PCM 2.0 Blarg [Flob]";
    let monster = "Monster [Ultimate Collection] By Blarg [Flob]/Monster [2004-2005] DVDRip x265 AC-3 DD 2.0 Blarg [Flob]";
    let code_geass = "[Foo] Code Geass - Hangyaku no Lelouch (Lelouch of the Rebellion) (Seasons 1-2 + Movies + OVAs) [BD 1080p][HEVC x265 10bit][Dual-Audio][Eng-Subs]";
    let files = [
        run(1, ["01", "02", "46"], &|n| format!("{chibi}/{n}.MP4")),
        // Not an episode, but the directory's movie.
        vec![video(4, &format!("{chibi}/Chibi Maruko-chan (1990).avi"), 93.0)],
        // A season in the directory, and in the name.
        run(10, ["01", "02", "03"], &|n| {
            format!("{food_wars}/Food Wars! Season 4 [Dual Audio]/Food Wars - {n}.mkv")
        }),
        run(13, ["01", "02", "03"], &|n| {
            format!("{food_wars}/Food Wars! Season 5 [Japanese Only]/Food Wars S5 - {n}.mkv")
        }),
        run(20, ["01v2", "02v2", "21"], &|n| {
            format!("[Fnarf] Jujutsu Kaisen (01-24) (1080p) [Batch]/Season 1/[Fnarf] Jujutsu Kaisen - {n} (1080p) [5A0F9C71].mkv")
        }),
        // `100` is the same in each.
        run(30, ["01", "02", "03"], &|n| {
            format!("{mob}/Mob Psycho 100 - {n} 1080p BDRip x265 PCM 2.0 Blarg [Flob].mkv")
        }),
        run(40, ["01", "02", "03"], &|n| {
            format!("{monster}/Monster - Chapter {n} - Title DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv")
        }),
        run(50, ["01", "02", "03"], &|n| {
            format!("One Punch Man (2015) [tbdbid-293088]/[bar] One Punch Man S2 [BD 1080p FLAC]/[bar] One Punch Man S2 - {n} [BD 1080p FLAC] [41F7C8E6].mkv")
        }),
        // Movies of a series.
        vec![
            video(60, "Toy Story 1995 2010 Trilogy Bluray 720p x264 ac3/Toy Story 1 1995.mp4", 81.0),
            video(61, "Toy Story 1995 2010 Trilogy Bluray 720p x264 ac3/Toy Story 2 1999.mp4", 92.0),
            video(62, "Toy Story 1995 2010 Trilogy Bluray 720p x264 ac3/Toy Story 3 2010.mp4", 103.0),
        ],
        run(70, ["01", "02", "03"], &|n| {
            format!("{code_geass}/[Foo] Code Geass Movies/[Foo] Code Geass Movie {n} - Title.mkv")
        }),
        run(73, ["01", "02", "03"], &|n| {
            format!("{code_geass}/[Foo] Code Geass OVAs/[Foo] Code Geass OVA04 - Akito The Exiled - {n}.mkv")
        }),
        // Movies named by numbers, of different years.
        vec![
            video(80, "300.2006.2160p.BluRay.REMUX.mkv", 117.0),
            video(81, "1917.2019.2160p.BluRay.REMUX.mkv", 119.0),
            video(82, "54.1998.1080p.BluRay.REMUX.mkv", 93.0),
            video(83, "12.Angry.Men.1957.UHD.BluRay.2160p.mkv", 96.0),
        ],
        // Titled after their numbers, without years.
        vec![
            video(84, "Classics/12 Angry Men.mkv", 96.0),
            video(85, "Classics/21 Grams.mkv", 124.0),
            video(86, "Classics/300 Spartans.mkv", 114.0),
        ],
        // A series' OVAs.
        run(98, ["01", "02", "03"], &|n| {
            format!("One Punch Man (2015) [tbdbid-293088]/[bar] One Punch Man S2 [BD 1080p FLAC]/[bar] One Punch Man S2 - OVA - {n} [BD 1080p FLAC] [7C1E9A02].mkv")
        }),
        // Numbered in tags, by the episodes of another series they're cut from.
        run(101, ["823-824", "825-826", "827-828"], &|n| {
            format!("OnePace/WholeCakeIsland/[One Pace][{n}] Whole Cake Island [720p][9B05D2C7].mkv")
        }),
        // Too few to be a run.
        vec![
            video(87, "Kill Bill/Kill Bill - Vol. 1.mkv", 111.0),
            video(88, "Kill Bill/Kill Bill - Vol. 2.mkv", 137.0),
        ],
        // Extras.
        run(90, ["01", "02", "03"], &|n| {
            format!("{mob}/Extras/Shorts/Mob Psycho 100 - Shorts {n}.mkv")
        }),
        vec![
            video(95, &format!("{food_wars}/OVA/Food Wars! S01 - OVA1.mkv"), 24.0),
            video(96, &format!("{food_wars}/OVA/Food Wars! S01 - OVA2.mkv"), 24.0),
            video(97, &format!("{food_wars}/OVA/Food Wars! S02 - OVA1.mkv"), 24.0),
        ],
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let catalog = catalog_without_kb(&files, &[]);

    let absolute = |ids: [i64; 3], numbers: [u32; 3]| -> Vec<(i64, Option<u32>, [u32; 2])> {
        ids.into_iter()
            .zip(numbers)
            .map(|(id, n)| (id, None, [n, n]))
            .collect()
    };
    let of_season = |season, ids: [i64; 3]| -> Vec<(i64, Option<u32>, [u32; 2])> {
        ids.into_iter()
            .zip(1..)
            .map(|(id, n)| (id, Some(season), [n, n]))
            .collect()
    };
    assert_eq!(
        series_episodes(&catalog),
        vec![
            ("chibi maruko-chan".into(), absolute([1, 2, 3], [1, 2, 46])),
            (
                "food wars".into(),
                [of_season(4, [10, 11, 12]), of_season(5, [13, 14, 15])].concat()
            ),
            (
                "jujutsu kaisen".into(),
                vec![
                    (20, Some(1), [1, 1]),
                    (21, Some(1), [2, 2]),
                    (22, Some(1), [21, 21])
                ]
            ),
            ("mob psycho 100".into(), absolute([30, 31, 32], [1, 2, 3])),
            ("monster".into(), absolute([40, 41, 42], [1, 2, 3])),
            ("one punch man".into(), of_season(2, [50, 51, 52])),
        ]
    );
    let movie_ids: Vec<i64> = movies(&catalog)
        .into_iter()
        .flat_map(|(_, files)| files.into_iter().map(|(id, _)| id))
        .collect();
    for id in [
        // Without a knowledge base, OVAs too.
        4, 60, 61, 62, 70, 71, 72, 73, 74, 75, 80, 81, 82, 83, 84, 85, 86, 87, 88, 98, 99, 100,
    ] {
        assert!(movie_ids.contains(&id), "file {id} isn't a movie");
    }
}

#[tokio::test]
async fn a_run_of_episodes_outranks_a_movie_only_as_long_as_one() {
    let chibi = "[Florp] Chibi Maruko-chan";
    let files = [
        video(1, &format!("{chibi}/01.MP4"), 21.2),
        video(2, &format!("{chibi}/02.MP4"), 21.2),
        video(3, &format!("{chibi}/03.MP4"), 21.2),
    ];
    // As high as a movie found by the file's title, `01`, ranks when only
    // its runtime says it's the movie: with the TMDB, runtime, closeness
    // and title bonuses.
    let rank = 1.0 + 2.0 + 3.0 + 1.0 + 1.5;
    let kb = FixedKb(vec![vec![
        Ranked::new(
            rank,
            ClaimPredicate::MovieExternalId(ExternalId::Tmdb(433182)),
        ),
        Ranked::new(rank, ClaimPredicate::MovieTitle("Ash 01".into())),
    ]]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "chibi maruko-chan".into(),
            vec![(1, None, [1, 1]), (2, None, [2, 2]), (3, None, [3, 3])]
        )]
    );
    assert_eq!(movies(&catalog), vec![]);
}

#[test]
fn a_directory_of_a_run_of_episodes_isnt_a_movies() {
    let dr_slump = "(frobnitz-b'ork) Dr. Slump & Arale-chan (Ep. 1-54)";
    let files = [
        video(
            1,
            &format!("{dr_slump}/Dr. Slump & Arale-chan 019 v2 [b'ork].mkv"),
            24.6,
        ),
        // The one the directory would take for its movie.
        video(
            2,
            &format!("{dr_slump}/Dr. Slump & Arale-chan 020 [6D2A0E5B] [b'ork].mkv"),
            24.7,
        ),
        video(
            3,
            &format!("{dr_slump}/Dr. Slump & Arale-chan 021 [b'ork].mkv"),
            24.6,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "dr slump & arale-chan".into(),
            vec![
                (1, None, [19, 19]),
                (2, None, [20, 20]),
                (3, None, [21, 21])
            ]
        )]
    );
    assert_eq!(movies(&catalog), vec![]);
}

#[test]
fn a_run_is_strong_evidence_of_episodes_only_as_long_as_they_are() {
    let files = [
        video(1, "Show/Show - 01.mkv", 24.0),
        video(2, "Show/Show - 02.mkv", 24.0),
        video(3, "Show/Show - 03.mkv", 24.0),
        // A series' movies, numbered alike.
        video(
            4,
            "Star Wars/Star Wars - 01 - The Phantom Menace.mkv",
            136.0,
        ),
        video(
            5,
            "Star Wars/Star Wars - 02 - Attack of the Clones.mkv",
            142.0,
        ),
        video(
            6,
            "Star Wars/Star Wars - 03 - Revenge of the Sith.mkv",
            140.0,
        ),
    ];
    let claims = run_heuristics(&files);

    let run_ranks: Vec<(i64, Option<f32>)> = (0..files.len())
        .flat_map(|subject| {
            let row_id = files[subject].1.row_id.0;
            let claims = claims.get(&subject).into_iter().flatten();
            claims
                .filter(|claim| claim.source == ClaimSource::Heuristic(Heuristic::NumberedRun))
                .map(move |claim| (row_id, claim.rank.as_number()))
                .take(1)
        })
        .collect();
    assert_eq!(
        run_ranks,
        vec![
            (1, Some(55.0)),
            (2, Some(55.0)),
            (3, Some(55.0)),
            (4, Some(6.7)),
            (5, Some(6.7)),
            (6, Some(6.7))
        ]
    );
}

#[test]
fn directories_above_a_run_name_the_series_it_could_be_a_season_of() {
    let files = [
        // Tagged differently, but a run of Whole Cake Island.
        video(
            1,
            "OnePace/WholeCakeIsland/[One Pace][823-824] Whole Cake Island 01 [720p][9B05D2C7].mkv",
            31.1,
        ),
        video(
            2,
            "OnePace/WholeCakeIsland/[One Pace][825-826] Whole Cake Island 02 [720p][38A6C5D2].mkv",
            30.0,
        ),
        video(
            3,
            "OnePace/WholeCakeIsland/[One Pace][827-828] Whole Cake Island 03 [720p][2E7C40B9].mkv",
            29.0,
        ),
        video(
            4,
            "OnePace/WanoAct2/[925-926] Wano 13 [1080p][A1D58F3C].mkv",
            28.0,
        ),
        video(
            5,
            "OnePace/WanoAct2/[926-927] Wano 14 [1080p][0C4E7A51].mkv",
            28.0,
        ),
        video(
            6,
            "OnePace/WanoAct2/[928-930] Wano 15 [1080p][E57D19F0].mkv",
            28.0,
        ),
    ];
    let claims = run_heuristics(&files);

    let series_names = |subject: SubjectIndex| -> Vec<(String, f32)> {
        claims[&subject]
            .iter()
            .filter_map(|claim| match &claim.pred {
                ClaimPredicate::TvSeriesName(name) => {
                    Some((name.clone(), claim.rank.as_number().unwrap()))
                }
                _ => None,
            })
            .collect()
    };
    let ranked = |names: &[(&str, f32)]| -> Vec<(String, f32)> {
        names
            .iter()
            .map(|&(name, rank)| (name.into(), rank))
            .collect()
    };
    assert_eq!(
        series_names(0),
        ranked(&[("one pace", 54.9), ("whole cake island", 55.0)])
    );
    assert_eq!(
        series_names(3),
        ranked(&[("wano act2", 54.9), ("one pace", 54.8), ("wano", 55.0)])
    );
    let episodes: Vec<_> = claims[&3]
        .iter()
        .filter(|claim| matches!(claim.pred, ClaimPredicate::AbsoluteEpisodeRange(_)))
        .map(|claim| claim.pred.clone())
        .collect();
    assert_eq!(
        episodes,
        vec![ClaimPredicate::AbsoluteEpisodeRange([13, 13])]
    );
}

/// One Pace and the drama named like one of its arcs, as TMDB lists them:
/// One Pace has a season named like the arc, which it knows no episodes of.
fn one_pace_kb() -> CatalogKb {
    let reverie = SeasonSummary {
        number: 34,
        name: "Reverie".into(),
        episode_count: 0,
    };
    CatalogKb {
        series: vec![
            kb_series(80588, "Reverie", None),
            SeriesRecord {
                seasons: vec![reverie],
                ..kb_series(336584, "One Pace", None)
            },
        ],
        ..CatalogKb::default()
    }
}

#[tokio::test]
async fn a_run_is_a_season_of_a_series_with_a_season_named_like_it() {
    let files = [
        video(
            1,
            "OnePace/Reverie/[One Pace][903-904] Reverie 01 [1080p][6F92B0E8].mkv",
            33.5,
        ),
        video(
            2,
            "OnePace/Reverie/[One Pace][905-906] Reverie 02 [1080p][B9F0427E].mkv",
            35.1,
        ),
        video(
            3,
            "OnePace/Reverie/[One Pace][907-908] Reverie 03 [1080p][2E7C40B9].mkv",
            28.6,
        ),
    ];
    let catalog = catalog_with_kb(&files, &[], &one_pace_kb()).await;

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series
                .episodes
                .iter()
                .map(|e| (e.file_id.0, e.season_number, e.episode_range))
                .collect();
            episodes.sort();
            (series.tmdb_id, series.name.as_str(), episodes)
        })
        .collect();
    assert_eq!(
        series,
        vec![(
            Some(336584),
            "One Pace",
            vec![
                (1, Some(34), [1, 1]),
                (2, Some(34), [2, 2]),
                (3, Some(34), [3, 3])
            ]
        )]
    );
    assert!(catalog.movies.is_empty());
}

/// A fraction after a number, e.g. `024.5`, numbers a special between two
/// episodes, as fansubs do, not episode 24: it's left to the specials
/// heuristics, and a knowledge base, to place.
#[test]
fn a_fractional_number_is_not_an_episode_in_a_run() {
    let slump = "(grumbler-n'blorbsubs) Dr. Slump & Arale-chan (Ep. 1-54)";
    let files = [
        video(
            1,
            &format!("{slump}/Dr. Slump & Arale-chan 024 [C98F8133] [n'blorbsubs].mkv"),
            24.0,
        ),
        video(
            2,
            &format!("{slump}/Dr. Slump & Arale-chan 025 [B3F0A1D2] [n'blorbsubs].mkv"),
            24.0,
        ),
        video(
            3,
            &format!("{slump}/Dr. Slump & Arale-chan 026 [0E8C7A61] [n'blorbsubs].mkv"),
            24.0,
        ),
        video(
            4,
            &format!("{slump}/Dr. Slump & Arale-chan 024.5 Special 1 [181E9280] [n'blorbsubs].mkv"),
            24.0,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let [series] = &catalog.tv_series[..] else {
        panic!("one series: {:?}", series_episodes(&catalog));
    };
    let mut episodes: Vec<_> = series
        .episodes
        .iter()
        .map(|e| (e.file_id.0, e.episode_range))
        .collect();
    episodes.sort();
    assert_eq!(episodes, vec![(1, [24, 24]), (2, [25, 25]), (3, [26, 26])]);
    assert!(series.extras.is_empty());
    assert!(
        catalog
            .unidentified_videos
            .contains(&FileMeasurementRowId(4))
    );
}

#[test]
fn a_run_numbered_in_tags_of_their_own_is_episodes_of_the_series_a_tag_names() {
    let run = |first_id: i64, name: &dyn Fn(u32) -> String| {
        (first_id..)
            .zip(1..=3)
            .map(|(id, number)| video(id, &name(number), 24.0))
            .collect::<Vec<_>>()
    };
    let files = [
        // The release group's tag, then the series'.
        run(1, &|n| {
            format!("[Blorp-Raws][Wumbo Quest][01-03][1080P]/[Blorp-Raws][Wumbo Quest][{n:02}][1080P][BDRip][HEVC-10bit][FLAC].mkv")
        }),
        // A year's tag, not a run though in tags of its own too, and a
        // genre's, farther from the number than the series'.
        run(10, &|n| {
            format!("Glimmer/[Fnord-Team][Donghua][Glimmer Peaks][2020][{n:02}][AVC][GB][1080P].mp4")
        }),
        // A release's tag before the group's.
        run(20, &|n| {
            format!("Zibble/[1080P][Blorp-Raws][Zibble][{n:02}v2][AC3].mkv")
        }),
        // The group's tag alone: the directory names the series.
        run(30, &|n| {
            format!("[Blorp-Raws] Yonder Lark [BDRip 1080p]/[Blorp-Raws][{n:02}][848x480].mkv")
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        series_episodes(&catalog),
        vec![
            (
                "glimmer peaks".into(),
                vec![(10, None, [1, 1]), (11, None, [2, 2]), (12, None, [3, 3])]
            ),
            (
                "wumbo quest".into(),
                vec![(1, None, [1, 1]), (2, None, [2, 2]), (3, None, [3, 3])]
            ),
            (
                "yonder lark".into(),
                vec![(30, None, [1, 1]), (31, None, [2, 2]), (32, None, [3, 3])]
            ),
            (
                "zibble".into(),
                vec![(20, None, [1, 1]), (21, None, [2, 2]), (22, None, [3, 3])]
            ),
        ]
    );
    assert_eq!(movies(&catalog), vec![]);
    // The year goes with the series' name, and the genre's tag is a name to
    // look up too, below it.
    assert!(active_claims(&catalog, 10).contains(&ClaimPredicate::TvSeriesYear(2020)));
    let (_, log) = catalog
        .audit_logs
        .iter()
        .find(|(id, _)| id.0 == 10)
        .unwrap();
    assert!(
        log.claims
            .iter()
            .any(|claim| claim.pred == ClaimPredicate::TvSeriesName("donghua".into()))
    );
}

#[test]
fn two_videos_numbered_in_tags_of_their_own_are_still_parts() {
    // Too few for a run: a concert's halves as likely as two episodes.
    let files = [1, 2].map(|n| {
        video(
            n,
            &format!("Live/[Blorp-Raws][Wumbo Live][{n:02}][1080P].mkv"),
            60.0,
        )
    });
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(series_episodes(&catalog), vec![]);
    for n in [1, 2] {
        let claims = active_claims(&catalog, n);
        assert!(
            claims.contains(&ClaimPredicate::Part(n as u32)),
            "{claims:?}"
        );
        assert!(
            !claims
                .iter()
                .any(|c| matches!(c, ClaimPredicate::AbsoluteEpisodeRange(_)))
        );
    }
}

#[test]
fn words_before_a_number_in_a_tag_of_its_own_name_the_series() {
    let files: Vec<_> = (1..=3)
        .map(|n| {
            video(
                n,
                &format!("Wumbo/[Blorp] Wumbo Quest [{n:02}][1080p][HEVC].mkv"),
                24.0,
            )
        })
        .collect();
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        series_episodes(&catalog),
        vec![(
            "wumbo quest".into(),
            vec![(1, None, [1, 1]), (2, None, [2, 2]), (3, None, [3, 3])]
        )]
    );
}
