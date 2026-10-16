//! Extras: which work's they are, by their names and directories.

use super::*;

#[test]
fn creditless_openings_and_extras_directories_are_series_extras() {
    let cg = "[Foo] Code Geass (Seasons 1-2) [BD 1080p]";
    let files = [
        video(
            1,
            &format!("{cg}/[Foo] Code Geass S1/[Foo] Code Geass - S01E01.mkv"),
            24.0,
        ),
        // Named with its series.
        video(
            2,
            &format!(
                "{cg}/[Foo] Code Geass Openings & Endings/[Foo] Code Geass - S01 NCOP 01a.mkv"
            ),
            1.5,
        ),
        // Of the series of the episodes near it.
        video(
            3,
            &format!("{cg}/[Foo] Code Geass Extras/[Foo] Code Geass - Stage 0.25.mkv"),
            10.0,
        ),
        video(4, "Cowboy Bebop/Season 1/Cowboy Bebop - S01E01.mkv", 24.0),
        video(5, "Cowboy Bebop/Season 1/NCED02.mkv", 1.5),
        video(6, "Cowboy Bebop/OP & ED/Tank!.mkv", 1.5),
        // A movie's extras stay the movie's.
        video(7, "Heat (1995)/Heat (1995).mkv", 170.0),
        video(8, "Heat (1995)/Extras/Making Of.mkv", 20.0),
        video(9, "The Help (2011) + Extras/The Help (2011).mkv", 146.0),
        video(
            10,
            "The Help (2011) + Extras/Featurettes/Deleted Scenes.mkv",
            9.6,
        ),
        // A movie titled like extras is still the main file.
        video(
            11,
            "Movies/Happy Endings (2005)/Happy Endings (2005).mkv",
            128.0,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.name.as_str(), episodes, extras)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            ("code geass", vec![1], vec![2, 3]),
            ("cowboy bebop", vec![4], vec![5, 6]),
        ]
    );
    assert_eq!(
        movies(&catalog),
        vec![
            ("happy endings".into(), vec![(11, Some(FileRole::Main))]),
            (
                "heat".into(),
                vec![(7, Some(FileRole::Main)), (8, Some(FileRole::Extra))]
            ),
            (
                "the help".into(),
                vec![(9, Some(FileRole::Main)), (10, Some(FileRole::Extra))]
            ),
        ]
    );
}

#[test]
fn extras_are_found_by_more_directory_names_and_tags() {
    let slump = "(frobnitz-b'ork) Dr. Slump & Arale-chan (Ep. 1-54)";
    let maruko = "[Florp] Chibi Maruko-chan";
    let monster = "Monster [Ultimate Collection] By Blarg [Flob]/Monster [2004-2005] DVDRip x265 AC-3 DD 2.0 Blarg [Flob]";
    let haruhi = "[Zork-Glorp] Haruhi [Wump_1080p]";
    let mut files = Vec::new();
    for n in 1..=3 {
        files.push(video(
            n,
            &format!("{slump}/Dr. Slump & Arale-chan 00{n} v2 (frobnitz).mkv"),
            24.0,
        ));
        files.push(video(10 + n, &format!("{maruko}/0{n}.MP4"), 21.0));
        files.push(video(
            20 + n,
            &format!(
                "{monster}/Monster - Chapter 0{n} - Title DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"
            ),
            23.9,
        ));
        files.push(video(
            30 + n,
            &format!("{haruhi}/[Zork-Glorp] Haruhi - S01E0{n} [Wump_1080p][x265_flac].mkv"),
            24.0,
        ));
    }
    files.extend([
        // A singular directory name, whole.
        video(
            4,
            &format!("{slump}/extra/Dr. Slump & Arale-chan 1981 Series Promo (frobnitz).mkv"),
            0.5,
        ),
        // Beside loose episodes, which aren't a movie's.
        video(14, &format!("{maruko}/OP & ED/~Yume Ippai.MP4"), 1.5),
        // Numbered like an extra, so not in a run of episodes either.
        video(
            24,
            &format!(
                "{monster}/Monster - Extra 1 - Origin DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"
            ),
            23.6,
        ),
        video(
            25,
            &format!("{monster}/Monster - Extra 2 - Book DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"),
            23.6,
        ),
        video(
            26,
            &format!("{monster}/Monster - Extra 3 - End DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"),
            23.6,
        ),
        // Zork-Glorp's directory of extras, and its tags.
        video(
            34,
            &format!("{haruhi}/SPs/[Zork-Glorp] Haruhi [Making01][Wump_720p][x265_aac].mkv"),
            14.7,
        ),
        video(
            35,
            &format!("{haruhi}/[Zork-Glorp] Haruhi [CM01][Wump_720p][x265_aac].mkv"),
            0.5,
        ),
    ]);
    let catalog = catalog_without_kb(&files, &[]);

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            episodes.sort();
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.name.as_str(), episodes, extras)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            ("chibi maruko-chan", vec![11, 12, 13], vec![14]),
            ("dr slump & arale-chan", vec![1, 2, 3], vec![4]),
            ("haruhi", vec![31, 32, 33], vec![34, 35]),
            ("monster", vec![21, 22, 23], vec![24, 25, 26]),
        ]
    );
    assert_eq!(movies(&catalog), vec![]);
}

#[test]
fn a_movie_directory_below_a_release_has_extras_too() {
    let haruhi = "[Zork-Glorp] Suzumiya Haruhi no Yuuutsu";
    let movie = format!("{haruhi}/[Zork-Glorp] Suzumiya Haruhi no Shoushitsu [Wump_1080p]");
    let series = format!("{haruhi}/[Zork-Glorp] Suzumiya Haruhi no Yuuutsu [Wump_1080p]");
    let opm = "One Punch Man (2015) [tbdbid-293088]/[bar] One Punch Man S2 [BD 1080p FLAC]";
    let mut files = vec![
        video(
            1,
            &format!(
                "{movie}/[Zork-Glorp] Suzumiya Haruhi no Shoushitsu [Wump_1080p][x265_flac].mkv"
            ),
            162.0,
        ),
        video(
            2,
            &format!(
                "{movie}/SPs/[Zork-Glorp] Suzumiya Haruhi no Shoushitsu [Making01][Wump_720p][x265_aac].mkv"
            ),
            29.3,
        ),
        video(
            3,
            &format!(
                "{movie}/SPs/[Zork-Glorp] Suzumiya Haruhi no Shoushitsu [Event02][Wump_720p][x265_aac].mkv"
            ),
            16.7,
        ),
        // Directories of episodes beside extras are a series', not a movie's.
        video(
            20,
            &format!(
                "{series}/SPs/[Zork-Glorp] Suzumiya Haruhi no Yuuutsu [Event][Wump_1080p][x265_aac].mkv"
            ),
            34.5,
        ),
        video(
            30,
            &format!("{opm}/[bar] One Punch Man S2 - OVA - 01 [BD 1080p FLAC] [7C1E9A02].mkv"),
            11.0,
        ),
        video(
            31,
            &format!("{opm}/Extras/[bar] One Punch Man S2 - NCOP [BD 1080p FLAC] [E3B14D8F].mkv"),
            1.5,
        ),
    ];
    for n in 1..=3 {
        files.push(video(
            10 + n,
            &format!("{series}/[Zork-Glorp] Suzumiya Haruhi no Yuuutsu S01E0{n} [Wump_1080p].mkv"),
            24.0,
        ));
        files.push(video(
            40 + n,
            &format!("{opm}/[bar] One Punch Man S2 - 0{n} [BD 1080p FLAC] [F86A3D14].mkv"),
            23.7,
        ));
    }
    // A series' directory isn't a movie's: none of its files is said to be
    // its main file, nor its extras one's, e.g. an episode's.
    let claims = run_heuristics(&files);
    let in_movie_dir: Vec<i64> = (0..files.len())
        .filter(|subject| {
            claims.get(subject).is_some_and(|claims| {
                claims
                    .iter()
                    .any(|claim| claim.source == ClaimSource::Heuristic(Heuristic::MovieDirectory))
            })
        })
        .map(|subject| files[subject].1.row_id.0)
        .collect();
    assert_eq!(in_movie_dir, vec![1, 2, 3]);

    let catalog = catalog_without_kb(&files, &[]);

    // Not the OVA beside the episodes, which isn't its directory's main
    // file, whatever it's found to be.
    let with_main: Vec<_> = movies(&catalog)
        .into_iter()
        .filter(|(_, files)| files.iter().any(|(_, role)| *role == Some(FileRole::Main)))
        .collect();
    assert_eq!(
        with_main,
        vec![(
            "suzumiya haruhi no shoushitsu".into(),
            vec![
                (1, Some(FileRole::Main)),
                (2, Some(FileRole::Extra)),
                (3, Some(FileRole::Extra))
            ]
        )]
    );
    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            episodes.sort();
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.name.as_str(), episodes, extras)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            ("one punch man", vec![41, 42, 43], vec![31]),
            ("suzumiya haruhi no yuuutsu", vec![11, 12, 13], vec![20]),
        ]
    );
}

#[test]
fn a_release_that_includes_extras_isnt_a_directory_of_them() {
    let bebop = "[Gronk] Cowboy Bebop [v3] [ITBD] [VFR] [Dual Audio] [Complete] [Extras] [x264]";
    let curb = "Curb Your Enthusiasm Season 1, 2, 3, 4, 5, 6, 7 & 8 + Extras DVDRip Zib";
    let help = "The Help (2011) + Extras (1080p BluRay x265 HEVC 10bit AAC 5.1 qwz72)";
    let code_geass = "[Foo] Code Geass - Hangyaku no Lelouch (Lelouch of the Rebellion) (Seasons 1-2 + Movies + OVAs) [BD 1080p][HEVC x265 10bit][Dual-Audio][Eng-Subs]";
    let files = [
        // What these releases include besides their episodes and movie.
        video(
            1,
            &format!("{bebop}/[Gronk] Cowboy Bebop - 02 [v3].mkv"),
            24.0,
        ),
        video(
            2,
            &format!(
                "{curb}/Season 1/Curb Your Enthusiasm Season 1 Episode 01 - The Pants Tent.avi"
            ),
            28.0,
        ),
        video(3, &format!("{help}/The Help (2011).mkv"), 146.0),
        // Directories of extras.
        video(4, &format!("{bebop}/Extras/BD Extras/Session 0.mkv"), 20.0),
        video(5, &format!("{curb}/Extras/Larry David Interview.avi"), 20.0),
        video(
            6,
            &format!("{code_geass}/[Foo] Code Geass Extras/Picture Drama.mkv"),
            5.0,
        ),
        // An extras word joined on to another.
        video(
            7,
            &format!("{code_geass}/[Foo] Code Geass Openings & Endings/Opening 1.mkv"),
            1.5,
        ),
        video(8, "Cowboy Bebop/OP & ED/Tank!.mkv", 1.5),
        video(9, "Monster/Creditless OP&ED/Opening.mkv", 1.5),
    ];
    let claims = run_heuristics(&files);

    let extras_by_directory: Vec<i64> = (0..files.len())
        .filter(|subject| {
            claims.get(subject).is_some_and(|claims| {
                claims.iter().any(|claim| {
                    claim.source == ClaimSource::Heuristic(Heuristic::ExtrasNameOrDirectory)
                        && claim.pred == ClaimPredicate::FileRole(FileRole::Extra)
                })
            })
        })
        .map(|subject| files[subject].1.row_id.0)
        .collect();
    assert_eq!(extras_by_directory, vec![4, 5, 6, 7, 8, 9]);
}

#[test]
fn an_extra_is_of_the_series_of_the_run_of_episodes_near_it() {
    let monster = "Monster [Ultimate Collection] By Blarg [Flob]/Monster [2004-2005] DVDRip x265 AC-3 DD 2.0 Blarg [Flob]";
    let chibi = "[Florp] Chibi Maruko-chan";
    let files = [
        video(
            1,
            &format!(
                "{monster}/Monster - Chapter 01 - Herr Dr. Tenma DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"
            ),
            23.9,
        ),
        video(
            2,
            &format!(
                "{monster}/Monster - Chapter 02 - Downfall DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"
            ),
            23.9,
        ),
        video(
            3,
            &format!(
                "{monster}/Monster - Chapter 03 - Murder Case DVDRip x265 AC-3 DD 2.0 Blarg [Flob].mkv"
            ),
            23.9,
        ),
        video(
            4,
            &format!("{monster}/Creditless OP&ED/Monster - Opening (Creditless).mkv"),
            1.4,
        ),
        video(
            5,
            &format!("{monster}/Creditless OP&ED/A Monster Without A Name by Franz Bonaparta.mkv"),
            3.8,
        ),
        video(6, &format!("{chibi}/01.MP4"), 21.2),
        video(7, &format!("{chibi}/02.MP4"), 21.2),
        video(8, &format!("{chibi}/03.MP4"), 21.2),
        video(9, &format!("{chibi}/OP & ED/~Odoru Pompokolin.MP4"), 1.4),
        // A series' movies aren't episodes, so its extras aren't a series'.
        video(
            10,
            "Movies/Star Wars/Star Wars - 01 - The Phantom Menace.mkv",
            136.0,
        ),
        video(
            11,
            "Movies/Star Wars/Star Wars - 02 - Attack of the Clones.mkv",
            142.0,
        ),
        video(
            12,
            "Movies/Star Wars/Star Wars - 03 - Revenge of the Sith.mkv",
            140.0,
        ),
        video(13, "Movies/Star Wars/Extras/The Beginning.mkv", 66.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.name.as_str(), series.episodes.len(), extras)
        })
        .collect();
    series.sort();
    assert_eq!(
        series[..2],
        [
            ("chibi maruko-chan", 3, vec![9]),
            ("monster", 3, vec![4, 5])
        ]
    );
    // Nor any other series'.
    assert!(
        series.iter().all(|(_, _, extras)| !extras.contains(&13)),
        "{series:?}"
    );
}

#[test]
fn an_extra_is_of_no_series_when_the_episodes_near_it_are_of_several() {
    let files = [
        video(1, "Mushishi/Mushishi - S01E01.mkv", 24.0),
        video(2, "Cowboy Bebop/Cowboy Bebop - S01E01.mkv", 24.0),
        // Only the data source's root has episodes, of two series.
        video(3, "Anime Extras/Picture Drama 1.mkv", 5.0),
    ];
    let claims = run_heuristics(&files);

    let names: Vec<_> = claims[&2]
        .iter()
        .filter(|claim| matches!(claim.pred, ClaimPredicate::TvSeriesName(_)))
        .collect();
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn another_series_in_a_season_release_is_its_extras() {
    let seven =
        "Seven.Worlds.One.Planet.S01.REPACK.2160p.Hybrid.UHD.BluRay.REMUX.HDR.HEVC.Atmos-Garply";
    let ippo = "Hajime.no.Ippo.S01.The.Fighting.REPACK.1080p.NF.WEB-DL.JP.EN.ES.AAC.2.0.x264-Corge";
    let files = [
        video(
            1,
            &format!(
                "{seven}/Seven.Worlds.One.Planet.S01E01.Antarctica.REPACK.2160p.Hybrid.UHD.BluRay.REMUX.HDR.HEVC.Atmos-Garply.mkv"
            ),
            51.9,
        ),
        video(
            2,
            &format!(
                "{seven}/Seven.Worlds.One.Planet.S01E02.Asia.REPACK.2160p.Hybrid.UHD.BluRay.REMUX.HDR.HEVC.Atmos-Garply.mkv"
            ),
            49.3,
        ),
        // Named after a series of their own.
        video(
            3,
            &format!("{seven}/On Location/On Location - E01 - Antarctica.mkv"),
            7.3,
        ),
        video(
            4,
            &format!("{seven}/On Location/On Location - E02 - Asia.mkv"),
            8.5,
        ),
        // The release's series under another name, as all its episodes are.
        video(
            5,
            &format!(
                "{ippo}/Hajime.no.Ippo.The.Fighting.S01E01.rerip.1080p.NF.WEB-DL.AAC2.0.x264.TRIAL.AUDIO-Waldo.mkv"
            ),
            23.7,
        ),
        video(
            6,
            &format!(
                "{ippo}/Hajime.no.Ippo.The.Fighting.S01E02.rerip.1080p.NF.WEB-DL.AAC2.0.x264.TRIAL.AUDIO-Waldo.mkv"
            ),
            23.7,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            episodes.sort();
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.name.as_str(), episodes, extras)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            ("hajime no ippo the fighting", vec![5, 6], vec![]),
            ("seven worlds one planet", vec![1, 2], vec![3, 4]),
        ]
    );
}

#[test]
fn extras_of_a_series_join_it() {
    let media = DataSourceUuid(Uuid::nil());
    let files = [
        video(1, "Code Geass - S01E01.mkv", 24.0),
        video(2, "Code Geass - NCOP 01.mkv", 1.5),
        // Named like an episode, but asserted to be an extra.
        video(3, "Code Geass - S01E13.5.mkv", 12.0),
    ];
    let name = ClaimPredicate::TvSeriesName("Code Geass".into());
    let extra = ClaimPredicate::FileRole(FileRole::Extra);
    let extras = ["Code Geass - NCOP 01.mkv", "Code Geass - S01E13.5.mkv"];
    let assertions = [
        assertion(media, &extras, name),
        assertion(media, &extras, extra),
    ];
    let catalog = catalog_without_kb(&files, &assertions);

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.name.as_str(), episodes, extras)
        })
        .collect();
    assert_eq!(series, vec![("Code Geass", vec![1], vec![2, 3])]);
    assert_eq!(movies(&catalog), vec![]);
}

#[tokio::test]
async fn extras_stay_with_their_movie_whatever_their_names_find() {
    let dir = "The Help (2011) + Extras";
    let files = [
        video(1, &format!("{dir}/The Help (2011).mkv"), 146.0),
        video(2, &format!("{dir}/Featurettes/Deleted Scenes.mkv"), 9.6),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(50014, "The Help", Some(2011), Some(146)),
            kb_movie(
                645483,
                "'Deep End': Remembering the Deleted Scenes",
                None,
                None,
            ),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    assert_eq!(
        movies_by_id(&catalog),
        vec![(
            Some(50014),
            vec![(1, Some(FileRole::Main)), (2, Some(FileRole::Extra))]
        )]
    );
    // The extra's own name wasn't searched for, so no claim came of it.
    let (_, log) = catalog.audit_logs.iter().find(|(id, _)| id.0 == 2).unwrap();
    let deep_end = ClaimPredicate::MovieExternalId(ExternalId::Tmdb(645483));
    assert!(!log.claims.iter().any(|claim| claim.pred == deep_end));
}

#[test]
fn a_creditless_video_beside_a_movie_is_its_extra() {
    let bebop = "Cowboy Bebop Knockin' on Heaven's Door";
    let files = [
        video(
            1,
            &format!("{bebop}/[Zazzle]Cowboy Bebop - Knockin' on Heaven's Door [2011 Release].mkv"),
            115.2,
        ),
        video(
            2,
            &format!(
                "{bebop}/[Zazzle] Cowboy Bebop - Knockin' on Heaven's Door 'Clean Opening Titles'.mkv"
            ),
            1.7,
        ),
        video(
            3,
            &format!("{bebop}/[Zazzle] Cowboy Bebop - Knockin' on Heaven's Door 'Clean ED'.mkv"),
            4.5,
        ),
        // Beside a series' episodes, it's still the series'.
        video(4, "Mushishi/Mushishi - S01E01.mkv", 24.0),
        video(5, "Mushishi/Mushishi - NCOP.mkv", 1.5),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        movies(&catalog),
        vec![(
            "cowboy bebop knockin on heavens door".into(),
            vec![
                (1, Some(FileRole::Main)),
                (2, Some(FileRole::Extra)),
                (3, Some(FileRole::Extra))
            ]
        )]
    );
    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            (series.name.as_str(), series.episodes.len(), extras)
        })
        .collect();
    assert_eq!(series, vec![("mushishi", 1, vec![5])]);
}

#[tokio::test]
async fn correcting_a_movie_keeps_its_extras() {
    let media = DataSourceUuid(Uuid::nil());
    let dir = "Tokyo Godfathers The Ultimate Enthusiast's Collected Edition";
    let main = format!("{dir}/Tokyo Godfathers USBD Remux.mkv");
    let extra = format!("{dir}/Extras/Making Of Tokyo Godfathers I.mkv");
    let files = [video(1, &main, 92.9), video(2, &extra, 22.2)];
    let kb = CatalogKb {
        movies: vec![kb_movie(13398, "Tokyo Godfathers", Some(2003), Some(92))],
        ..CatalogKb::default()
    };
    // As the movie page asserts it: about all of the movie's files.
    let tmdb = ClaimPredicate::MovieExternalId(ExternalId::Tmdb(13398));
    let assertions = [assertion(media, &[&main, &extra], tmdb)];
    let catalog = catalog_with_kb(&files, &assertions, &kb).await;

    assert_eq!(
        movies_by_id(&catalog),
        vec![(
            Some(13398),
            vec![(1, Some(FileRole::Main)), (2, Some(FileRole::Extra))]
        )]
    );
}

#[test]
fn an_asserted_episode_outranks_being_in_a_directory_of_extras() {
    let media = DataSourceUuid(Uuid::nil());
    let wire = "The Wire (2002) Season 1-5 S01-S05 (1080p BluRay x265 HEVC 10bit AAC 5.1 Blivet)";
    let special = format!("{wire}/Featurettes/Season 4/It's All Connected.mkv");
    let files = [
        video(
            1,
            &format!("{wire}/Season 4/The Wire - S04E01 - Boys of Summer.mkv"),
            58.0,
        ),
        video(2, &special, 28.8),
        video(
            3,
            &format!("{wire}/Featurettes/Season 4/The Game Is Real.mkv"),
            30.0,
        ),
    ];
    let assertions = [
        assertion(media, &[&special], ClaimPredicate::Season(0)),
        assertion(media, &[&special], ClaimPredicate::EpisodeRange([1, 1])),
    ];
    let catalog = catalog_without_kb(&files, &assertions);

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
            let extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            (series.name.as_str(), episodes, extras)
        })
        .collect();
    // The other featurette, unasserted, stays the series' extra.
    assert_eq!(
        series,
        vec![(
            "the wire",
            vec![(1, Some(4), [1, 1]), (2, Some(0), [1, 1])],
            vec![3]
        )]
    );
}

#[tokio::test]
async fn an_extra_asserted_a_series_id_joins_that_series() {
    let media = DataSourceUuid(Uuid::nil());
    let dir = "[Zork-Glorp] Suzumiya Haruhi no Yuuutsu";
    let episode = format!("{dir}/Suzumiya Haruhi no Yuuutsu - S01E01.mkv");
    // Named after a series of its own, which TMDB doesn't find.
    let creditless = format!(
        "{dir}/Suzumiya Haruhi-chan no Yuuutsu [Wump_720p]/SPs/\
         Suzumiya Haruhi-chan no Yuuutsu [NCOP][Wump_720p].mkv"
    );
    let files = [video(1, &episode, 24.0), video(2, &creditless, 1.1)];
    let kb = series_kb(vec![(1655, "The Melancholy of Haruhi Suzumiya", 2006)]);
    let tmdb = ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(1655));
    let assertions = [
        assertion(media, &[&episode], tmdb.clone()),
        assertion(media, &[&creditless], tmdb),
    ];
    let catalog = catalog_with_kb(&files, &assertions, &kb).await;

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            let extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            (series.tmdb_id, episodes, extras)
        })
        .collect();
    assert_eq!(series, vec![(Some(1655), vec![1], vec![2])]);
}

#[test]
fn only_videos_are_extras() {
    let help = "The Help (2011) + Extras (1080p BluRay x265 HEVC 10bit AAC 5.1 qwz72)";
    let soundtrack = "Extras/Soundtrack";
    let track = {
        let (path_id, mut facts) =
            video(3, &format!("{help}/{soundtrack}/01. Main Title.flac"), 3.5);
        facts.media.as_mut().unwrap().video.clear();
        (path_id, facts)
    };
    let files = [
        video(1, &format!("{help}/The Help (2011).mkv"), 146.0),
        video(2, &format!("{help}/Featurettes/Deleted Scenes.mkv"), 9.6),
        track,
        file(4, &format!("{help}/{soundtrack}/Artwork/Front.jpg")),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        movies(&catalog),
        vec![(
            "the help".into(),
            vec![(1, Some(FileRole::Main)), (2, Some(FileRole::Extra))]
        )]
    );
}

#[tokio::test]
async fn extras_join_their_main_files_series_if_its_an_episode() {
    let bebop = "[Gronk] Cowboy Bebop [v3] [ITBD] [VFR] [Dual Audio] [Complete] [Extras] [x264]";
    let help = "The Help (2011) + Extras (1080p BluRay x265 HEVC 10bit AAC 5.1 qwz72)";
    let files = [
        // A directory of a series, which is also taken for a movie's with
        // one of these as its main file.
        video(
            1,
            &format!("{bebop}/[Gronk] Cowboy Bebop - 01 [v3].mkv"),
            24.7,
        ),
        video(
            2,
            &format!("{bebop}/[Gronk] Cowboy Bebop - 02 [v3].mkv"),
            24.6,
        ),
        video(
            3,
            &format!("{bebop}/[Gronk] Cowboy Bebop - 03 [v3].mkv"),
            24.5,
        ),
        video(
            4,
            &format!("{bebop}/Extras/NCOP/[Gronk] Cowboy Bebop - NCOP.mkv"),
            1.5,
        ),
        video(5, &format!("{bebop}/Extras/BD Extras/Session 0.mkv"), 20.0),
        // A movie's extras still join its movie.
        video(6, &format!("{help}/The Help (2011).mkv"), 146.0),
        video(7, &format!("{help}/Featurettes/Deleted Scenes.mkv"), 9.6),
    ];
    let kb = series_kb(vec![(30991, "Cowboy Bebop", 1998)]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let mut episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
            episodes.sort();
            let mut extras: Vec<_> = series.extras.iter().map(|id| id.0).collect();
            extras.sort();
            (series.tmdb_id, episodes, extras)
        })
        .collect();
    assert_eq!(series, vec![(Some(30991), vec![1, 2, 3], vec![4, 5])]);
    assert_eq!(
        movies(&catalog),
        vec![(
            "the help".into(),
            vec![(6, Some(FileRole::Main)), (7, Some(FileRole::Extra))]
        )]
    );
}

#[test]
fn a_run_numbered_in_tags_lends_its_series_to_the_extras_and_specials_beside_it() {
    let release = "[Blorp-Raws][Wumbo Quest][01-12TV+SP][1080P][BDRip]";
    let paths: Vec<String> =
        (1..=3)
            .map(|n| format!("{release}/[Blorp-Raws][Wumbo Quest][{n:02}][1080P][BDRip].mkv"))
            .chain([
                format!("{release}/NCOP&NCED/[Blorp-Raws][Wumbo Quest][NCED1][1080P][BDRip].mkv"),
                // A full opening among creditless ones: not a movie of its folder.
                format!("{release}/NCOP/[Blorp-Raws][Wumbo Quest][NCOP1][1080P][BDRip].mkv"),
                format!("{release}/NCOP/[Blorp-Raws][Wumbo Quest][NCOP2][1080P][BDRip].mkv"),
                format!("{release}/NCOP/[Blorp-Raws][Wumbo Quest][OP Full][1080P][BDRip].mkv"),
                format!("{release}/menu/[Blorp-Raws][Wumbo Quest][menu][01][1080P][BDRip].mkv"),
                format!("{release}/PV/[Blorp-Raws][Wumbo Quest][PV][01][1080P][BDRip].mkv"),
            ])
            .chain((1..=3).map(|n| {
                format!("{release}/SP/[Blorp-Raws][Wumbo Quest][SP][{n:02}][1080P][BDRip].mkv")
            }))
            .chain((1..=3).map(|n| {
                format!("{release}/[Blorp-Raws][Wumbo Quest][OVA][{n:02}][1080P][BDRip].mkv")
            }))
            // Bonus videos numbered beside the episodes, in a word for them
            // that nothing knows: not a second series.
            .chain((1..=3).map(|n| {
                format!("{release}/[Blorp-Raws][Wumbo Quest][Glimpses][{n:02}][1080P][BDRip].mkv")
            }))
            .collect();
    // Measured as episodes are, and unmeasured, as when ffprobe can't read
    // them: either way, the run is the series'.
    for minutes in [Some(24.0), None] {
        let files: Vec<_> = (0..)
            .zip(&paths)
            .map(|(id, path)| match minutes {
                Some(minutes) => video(id, path, minutes),
                None => {
                    let (path, mut facts) = video(id, path, 0.0);
                    facts
                        .media
                        .as_mut()
                        .unwrap()
                        .container
                        .as_mut()
                        .unwrap()
                        .duration_ms = None;
                    (path, facts)
                }
            })
            .collect();
        let catalog = catalog_without_kb(&files, &[]);
        // The run alone is episodes: not `SP` or `OVA`, whose numbers aren't
        // the series' episodes.
        assert_eq!(
            series_episodes(&catalog),
            vec![(
                "wumbo quest".into(),
                vec![(0, None, [1, 1]), (1, None, [2, 2]), (2, None, [3, 3])]
            )],
            "{minutes:?}"
        );
        assert_eq!(movies(&catalog), vec![], "{minutes:?}");
        for extra in 3..=8 {
            let claims = active_claims(&catalog, extra);
            assert!(
                claims.contains(&ClaimPredicate::TvSeriesName("wumbo quest".into()))
                    && claims.contains(&ClaimPredicate::FileRole(FileRole::Extra)),
                "{minutes:?} {}: {claims:?}",
                paths[extra as usize]
            );
        }
        for extra in 15..=17 {
            let claims = active_claims(&catalog, extra);
            assert!(
                claims.contains(&ClaimPredicate::TvSeriesName("wumbo quest".into()))
                    && claims.contains(&ClaimPredicate::FileRole(FileRole::Extra)),
                "{minutes:?} {}: {claims:?}",
                paths[extra as usize]
            );
        }
        for special in 9..=14 {
            let claims = active_claims(&catalog, special);
            assert!(
                claims.contains(&ClaimPredicate::SpecialOf("wumbo quest".into())),
                "{minutes:?} {}: {claims:?}",
                paths[special as usize]
            );
        }
    }
}

#[test]
fn a_movies_bonus_videos_below_it_are_its_extras() {
    let trilogy = "[Blorp-Raws][Wumbo Trilogy][2160P][BDRip]";
    let zibble = "[Blorp-Raws][Zibble][1080P][BDRip]";
    let files = [
        video(
            1,
            &format!("{trilogy}/[Blorp-Raws][4K][Wumbo I][2160P][BDRip].mkv"),
            137.0,
        ),
        video(
            2,
            &format!("{trilogy}/[Blorp-Raws][4K][Wumbo II][2160P][BDRip].mkv"),
            133.0,
        ),
        video(
            3,
            &format!("{trilogy}/[Blorp-Raws][4K][Wumbo III][2160P][BDRip].mkv"),
            141.0,
        ),
        // Each of the trilogy's bonus videos names the movie it's of.
        video(
            4,
            &format!("{trilogy}/PV/[Blorp-Raws][4K][Wumbo II][PV][01][2160P].mkv"),
            1.0,
        ),
        video(
            5,
            &format!("{trilogy}/PV/[Blorp-Raws][4K][Wumbo II][PV][02][2160P].mkv"),
            1.0,
        ),
        video(
            6,
            &format!("{trilogy}/PV/[Blorp-Raws][4K][Wumbo III][PV][01][2160P].mkv"),
            1.0,
        ),
        video(
            7,
            &format!("{trilogy}/menu/[Blorp-Raws][4K][Wumbo I][menu][2160P].mkv"),
            1.0,
        ),
        // A movie's only one: whatever it's named.
        video(
            10,
            &format!("{zibble}/[Blorp-Raws][Zibble][1080P][BDRip].mkv"),
            95.0,
        ),
        video(
            11,
            &format!("{zibble}/NCED/[Blorp-Raws][Zibble Song][NCED][1080P].mkv"),
            2.0,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);
    let extra = Some(FileRole::Extra);
    // Nothing says the movies are main files, though their extras are theirs.
    let main = None;
    assert_eq!(
        movies(&catalog),
        vec![
            ("wumbo i".into(), vec![(1, main), (7, extra)]),
            ("wumbo ii".into(), vec![(2, main), (4, extra), (5, extra)]),
            ("wumbo iii".into(), vec![(3, main), (6, extra)]),
            ("zibble".into(), vec![(10, main), (11, extra)]),
        ]
    );
}
