//! Specials: videos of a series that aren't its episodes.

use super::*;

#[test]
fn a_video_near_a_series_episodes_may_be_its_special() {
    let taskmaster = "Taskmaster.S01-S16+Specials.1080p.Web.AAC.HEVC.x265-Foobar";
    let maruko = "[Florp] Chibi Maruko-chan";
    let untold = "Oliver.Stones.Untold.History.of.the.United.States";
    let wire = "The Wire (2002) Season 1-5 S01-S05 (1080p BluRay x265 HEVC 10bit AAC 5.1 Blivet)";
    let opm = "One Punch Man (2015) [tbdbid-293088]/[bar] One Punch Man S2 [BD 1080p FLAC]";
    let mut files = vec![
        video(
            1,
            &format!(
                "{taskmaster}/New Year Treat/Taskmasters.New.Year.Treat.2022.1080p.Web.HEVC.x265-Foobar.mkv"
            ),
            47.2,
        ),
        video(
            2,
            &format!("{maruko}/Special/Special (1991) - Maruko's Wasted Summer.mp4"),
            35.3,
        ),
        video(
            3,
            &format!(
                "{untold}/{untold}.S01.Specials.720p.Bluray.DTS.5.1.x264-WIBBLE/oliver.stones.untold.history.of.the.united.states.s01.bonus.prologue.chapter.a.720p.bluray.x264-wibble.mkv"
            ),
            57.7,
        ),
        // A series' movie is one too, for all its heuristics know: its
        // titles finding a movie keep it one.
        video(
            4,
            &format!("{maruko}/Movies/Chibi Maruko-chan (1990).mp4"),
            94.0,
        ),
        // Not an extra, which is the series' already.
        video(
            5,
            &format!("{wire}/Featurettes/Season 4/The Game Is Real.mkv"),
            30.0,
        ),
        // Nor an OVA, a special by its own name, nor a sample.
        video(
            6,
            &format!("{opm}/[bar] One Punch Man S2 - OVA - 01 [BD 1080p FLAC] [7C1E9A02].mkv"),
            11.0,
        ),
        video(7, &format!("{maruko}/Sample/sample.mp4"), 0.5),
        // Nor a part, which its knowledge base numbers as an episode.
        video(
            8,
            &format!(
                "{taskmaster}/Champion of Champions/Season 1/Taskmaster.Champion.Of.Champions.Part.1.Wiley.Giraffe.Blower.1080p.Web.HEVC.x265-Foobar.mkv"
            ),
            44.9,
        ),
        video(
            9,
            &format!(
                "{taskmaster}/Champion of Champions/Season 2/Taskmaster-Champion.Of.Champions.S02E01.The.Alpine.Darling.1080p.Web.HEVC.x265-Foobar..mkv"
            ),
            45.0,
        ),
    ];
    for n in 1..=3 {
        files.extend([
            video(
                10 + n,
                &format!("{taskmaster}/Series 1/Taskmaster.S01E0{n}.1080p.Web.AAC.HEVC.x265-Foobar.mkv"),
                46.0,
            ),
            video(20 + n, &format!("{maruko}/0{n}.MP4"), 21.0),
            video(
                30 + n,
                &format!("{untold}/{untold}.S01.1080i.BluRay.REMUX.AVC.DTS-HD.MA.5.1-Plugh/{untold}.S01E0{n}.Chapter.{n}.1080i.BluRay.REMUX.AVC.DTS-HD.MA.5.1-Plugh.mkv"),
                58.0,
            ),
            video(
                40 + n,
                &format!("{wire}/Season 4/The Wire (2002) - S04E0{n} - Title (1080p BluRay x265 Blivet).mkv"),
                58.0,
            ),
            video(
                50 + n,
                &format!("{opm}/[bar] One Punch Man S2 - 0{n} [BD 1080p FLAC] [F86A3D14].mkv"),
                23.7,
            ),
        ]);
    }
    let claims = run_heuristics(&files);
    let files = &files;

    let mut specials: Vec<(i64, &str)> = (0..files.len())
        .flat_map(|subject| {
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter_map(move |claim| match (&claim.source, &claim.pred) {
                    (
                        ClaimSource::Heuristic(Heuristic::SpecialOfNearbySeries),
                        ClaimPredicate::SpecialOf(series),
                    ) => Some((files[subject].1.row_id.0, series.as_str())),
                    _ => None,
                })
        })
        .collect();
    specials.sort();
    assert_eq!(
        specials,
        vec![
            (1, "taskmaster"),
            (2, "chibi maruko-chan"),
            (3, "oliver stones untold history of the united states"),
            (4, "chibi maruko-chan"),
        ]
    );
}

/// A series' TMDB ID and name, and its episodes' and extras' row IDs.
type SeriesFiles = (Option<u32>, String, Vec<i64>, Vec<i64>);

#[tokio::test]
async fn an_ova_not_found_as_a_movie_is_a_special_of_its_series() {
    let code_geass = "[Foo] Code Geass - Hangyaku no Lelouch (Lelouch of the Rebellion) (Seasons 1-2 + Movies + OVAs) [BD 1080p][HEVC x265 10bit][Dual-Audio][Eng-Subs]";
    let food_wars = "[Fnord Bleep] Food Wars! (Shokugeki no Soma) S1 - S5 [Full Series][BD][Dual Audio][HEVC 10bit x265][AAC][Eng Sub]";
    let files = [
        // Found as a movie.
        video(
            1,
            &format!(
                "{code_geass}/[Foo] Code Geass OVAs/[Foo] Code Geass OVA03 - Nunnally in Wonderland.mkv"
            ),
            28.0,
        ),
        // Of the series the episodes that name it so were found to be.
        video(
            2,
            &format!("{food_wars}/Food Wars! Season 3 [Dual Audio]/Food Wars! S3 - 01.mkv"),
            24.0,
        ),
        video(
            3,
            &format!("{food_wars}/Food Wars! Season 3 [Dual Audio]/Food Wars! S3 - 02.mkv"),
            24.0,
        ),
        video(
            4,
            &format!("{food_wars}/Food Wars! Season 3 [Dual Audio]/Food Wars! S3 - 03.mkv"),
            24.0,
        ),
        video(
            5,
            &format!("{food_wars}/OVA/Food Wars! S01 - OVA1.mkv"),
            25.1,
        ),
        // Named by its directory, as a season names no series.
        video(
            6,
            "Mob.Psycho/Mob Psycho 100 S02+OVA 1080p Dual Audio BD Remux-Quux/S02OVA-A Healing Trip that Warms the Heart.mkv",
            24.2,
        ),
    ];
    let kb = CatalogKb {
        movies: vec![kb_movie(
            1370604,
            "Code Geass: Hangyaku no Lelouch - Nunnally in Wonderland",
            Some(2012),
            None,
        )],
        ..series_kb(vec![
            (62273, "Food Wars! Shokugeki no Soma", 2015),
            (67075, "Mob Psycho 100", 2016),
        ])
    };
    let series_files = |catalog: &NewCatalog| -> Vec<SeriesFiles> {
        let mut series: Vec<_> = catalog
            .tv_series
            .iter()
            .map(|series| {
                let mut episodes: Vec<_> = series.episodes.iter().map(|e| e.file_id.0).collect();
                episodes.sort();
                let extras = series.extras.iter().map(|id| id.0).collect();
                (series.tmdb_id, series.name.clone(), episodes, extras)
            })
            .collect();
        series.sort();
        series
    };

    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert_eq!(
        series_files(&catalog),
        vec![
            (
                Some(62273),
                "Food Wars! Shokugeki no Soma".into(),
                vec![2, 3, 4],
                vec![5]
            ),
            (Some(67075), "Mob Psycho 100".into(), vec![], vec![6]),
        ]
    );
    assert_eq!(
        movies_by_id(&catalog),
        vec![(Some(1370604), vec![(1, None)])]
    );

    // Without a knowledge base, nothing says they aren't movies.
    let catalog = catalog_without_kb(&files, &[]);
    let movie_files: Vec<i64> = movies(&catalog)
        .into_iter()
        .flat_map(|(_, files)| files.into_iter().map(|(id, _)| id))
        .collect();
    for id in [1, 5, 6] {
        assert!(movie_files.contains(&id), "file {id} isn't a movie");
    }
}
