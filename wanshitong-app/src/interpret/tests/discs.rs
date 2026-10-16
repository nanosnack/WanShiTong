//! Discs: folders and images of Blu-rays and DVDs.

use super::*;

#[test]
fn a_disc_is_its_movies_main_file_by_its_folders_name() {
    let dvd = "Keep.On.Keepin.On.2014.NTSC.DVDR-Frotz.AC3.2.0-NOGRP (13)";
    let blu_ray = "Do.the.Right.Thing.1989.2160p.BluRay";
    let files = [
        disc(
            1,
            &format!("{dvd}/VIDEO_TS (13)/VIDEO_TS.IFO"),
            DiscKind::Dvd,
            dvd,
            85.5,
        ),
        disc(
            2,
            &format!("{blu_ray}/BDMV/index.bdmv"),
            DiscKind::BluRay,
            blu_ray,
            119.8,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        movies(&catalog),
        vec![
            ("do the right thing".into(), vec![(2, Some(FileRole::Main))]),
            ("keep on keepin on".into(), vec![(1, Some(FileRole::Main))]),
        ]
    );

    // Its longest title gives TMDB a runtime to match against.
    let durations: Vec<_> = active_claims(&catalog, 1)
        .into_iter()
        .filter(|pred| matches!(pred, ClaimPredicate::Duration(_)))
        .collect();
    assert_eq!(
        durations,
        vec![ClaimPredicate::Duration(Duration::from_secs(85 * 60 + 30))]
    );
}

#[test]
fn a_disc_image_is_named_like_a_disc_folder() {
    let wind = "AnimeMovies/The.Wind.Rises.2013.Blu-ray.1080p.AVC.LPCM.2.0-Qux.iso";
    let heat = "Heat (1995) 1080p BluRay";
    let files = [
        disc(1, wind, DiscKind::BluRay, wind, 126.5),
        disc(
            2,
            &format!("{heat}/Disc 1.iso"),
            DiscKind::BluRay,
            &format!("{heat}/Disc 1.iso"),
            85.0,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);
    assert_eq!(
        movies(&catalog),
        vec![
            ("heat".into(), vec![(2, Some(FileRole::Main))]),
            ("the wind rises".into(), vec![(1, Some(FileRole::Main))]),
        ]
    );
}

#[test]
fn a_disc_identified_as_a_series_is_its_disc() {
    let media = DataSourceUuid(Uuid::nil());
    let root = "Oliver.Stones.Untold.History.S01.1080i.Blu-ray/D2";
    let index = format!("{root}/BDMV/index.bdmv");
    let files = [disc(1, &index, DiscKind::BluRay, root, 58.0)];
    let assertions = [
        assertion(
            media,
            &[&index],
            ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(46353)),
        ),
        assertion(
            media,
            &[&index],
            ClaimPredicate::TvSeriesName("Untold History".into()),
        ),
    ];
    let catalog = catalog_without_kb(&files, &assertions);
    // Not a movie, though its folder was taken for one, nor an episode.
    assert!(catalog.movies.is_empty());
    assert_eq!(catalog.tv_series.len(), 1);
    let series = &catalog.tv_series[0];
    assert!(series.episodes.is_empty() && series.extras.is_empty());
    assert_eq!(series.discs, [FileMeasurementRowId(1)]);
}

#[tokio::test]
async fn a_disc_of_a_set_is_named_by_the_sets_folder() {
    let untold =
        "Oliver.Stones.Untold.History.Of.The.United.States.S01.1080i.Blu-ray.AVC.DD2.0-Snarf";
    let heat = "Heat (1995) 1080p BluRay";
    let mut files: Vec<_> = (1..=3)
        .map(|n| {
            let root = format!("{untold}/D{n}");
            disc(
                n,
                &format!("{root}/BDMV/index.bdmv"),
                DiscKind::BluRay,
                &root,
                43.1,
            )
        })
        .collect();
    files.extend((1..=2).map(|n| {
        let root = format!("{heat}/Disc {n}");
        disc(
            10 + n,
            &format!("{root}/BDMV/index.bdmv"),
            DiscKind::BluRay,
            &root,
            85.0,
        )
    }));
    let kb = series_kb(vec![(
        46353,
        "Oliver Stones Untold History of the United States",
        2012,
    )]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    // A season release's discs are its series', not movies named `D1`.
    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let discs: Vec<_> = series.discs.iter().map(|id| id.0).collect();
            (series.tmdb_id, discs)
        })
        .collect();
    assert_eq!(series, vec![(Some(46353), vec![1, 2, 3])]);
    assert_eq!(
        movies(&catalog),
        vec![(
            "heat".into(),
            vec![(11, Some(FileRole::Main)), (12, Some(FileRole::Main))]
        )]
    );
}

#[test]
fn an_unidentified_disc_is_one_unidentified_video() {
    let media = DataSourceUuid(Uuid::nil());
    let index = "Do.the.Right.Thing.1989.2160p.BluRay/BDMV/index.bdmv";
    let files = [disc(
        1,
        index,
        DiscKind::BluRay,
        "Do.the.Right.Thing.1989.2160p.BluRay",
        119.8,
    )];
    let catalog = catalog_without_kb(&files, &[]);
    let unidentified: Vec<_> = catalog.unidentified_videos.iter().map(|id| id.0).collect();
    assert_eq!(unidentified, [1]);
    assert_eq!(catalog.videos, [FileMeasurementRowId(1)]);

    let assertions = [assertion(
        media,
        &[index],
        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(925)),
    )];
    let catalog = catalog_without_kb(&files, &assertions);
    assert!(catalog.unidentified_videos.is_empty());
}

#[tokio::test]
async fn discs_whose_folders_end_with_their_numbers_are_a_set() {
    let wumbo = "Wumbo.Quest.S01.1080p.Blu-ray.AVC.DTS-HD.MA.2.0-Snarf";
    let zibble = "Zibble S01-S02 NTSC 6xDVD9/Zibble Season 01";
    let files: Vec<_> = (1..=2)
        .map(|n| {
            let root = format!("{wumbo}/WUMBO_D{n}");
            let index = format!("{root}/BDMV/index.bdmv");
            disc(n, &index, DiscKind::BluRay, &root, 180.0)
        })
        .chain((1..=3).map(|n| {
            // Another tag after the number.
            let root = format!("{zibble}/ZIBBLE_S1_D{n}_B5");
            let index = format!("{root}/VIDEO_TS/VIDEO_TS.IFO");
            disc(10 + n, &index, DiscKind::Dvd, &root, 150.0)
        }))
        .collect();
    let kb = series_kb(vec![(101, "Wumbo Quest", 1998), (102, "Zibble", 1971)]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let mut series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let discs: Vec<_> = series.discs.iter().map(|id| id.0).collect();
            (series.tmdb_id, discs)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![(Some(101), vec![1, 2]), (Some(102), vec![11, 12, 13])]
    );
    assert_eq!(movies(&catalog), vec![]);

    // A set's folder named only in tags names nothing, so each disc's own
    // folder does, as it would alone.
    let files: Vec<_> = (1..=2)
        .map(|n| {
            let root = format!("[BDMV][Yonder Lark][BOX][JP]/YONDER_LARK_D{n}");
            let index = format!("{root}/BDMV/index.bdmv");
            disc(n, &index, DiscKind::BluRay, &root, 100.0)
        })
        .collect();
    let catalog = catalog_without_kb(&files, &[]);
    for n in 1..=2 {
        assert!(
            active_claims(&catalog, n)
                .contains(&ClaimPredicate::MovieTitle(format!("yonder lark d{n}"))),
            "{:?}",
            active_claims(&catalog, n)
        );
    }
}

#[test]
fn a_disc_folder_named_with_two_titles_claims_both() {
    let root = "The Glimmer AKA La lueur 1990 1080p FRA Blu-ray AVC DTS-HD MA 2.0";
    let files = [disc(
        1,
        &format!("{root}/BDMV/index.bdmv"),
        DiscKind::BluRay,
        root,
        101.0,
    )];
    let claims = run_heuristics(&files);
    let titles: Vec<&ClaimPredicate> = claims[&0]
        .iter()
        .map(|claim| &claim.pred)
        .filter(|pred| matches!(pred, ClaimPredicate::MovieTitle(_)))
        .collect();
    assert_eq!(
        titles,
        [
            &ClaimPredicate::MovieTitle("the glimmer".into()),
            &ClaimPredicate::MovieTitle("la lueur".into()),
        ]
    );
}

#[tokio::test]
async fn a_set_of_discs_is_looked_up_by_its_name_and_each_discs_own() {
    let wumbo = "[BDMV] Wumbo Quest";
    let zibble = "Zibble Collection";
    let files: Vec<_> = (1..=5)
        .map(|n| {
            // Named alike but for their numbers, without a word for discs.
            let root = format!("{wumbo}/WUMBO_QUEST_{n}");
            let index = format!("{root}/BDMV/index.bdmv");
            disc(n, &index, DiscKind::BluRay, &root, 140.0)
        })
        .chain((1..=3).map(|n| {
            // Named for their numbers alone, which says nothing else.
            let root = format!("Glimmer Peaks Box/Disc {n}");
            let index = format!("{root}/BDMV/index.bdmv");
            disc(20 + n, &index, DiscKind::BluRay, &root, 100.0)
        }))
        .chain((1..=2).map(|n| {
            // A set of movies, one a disc.
            let root = format!("{zibble}/Zibble Vol {n} 2004");
            let index = format!("{root}/BDMV/index.bdmv");
            disc(10 + n, &index, DiscKind::BluRay, &root, 110.0)
        }))
        .collect();
    let kb = CatalogKb {
        movies: vec![
            kb_movie(201, "Zibble Vol 1", Some(2004), None),
            kb_movie(202, "Zibble Vol 2", Some(2004), None),
        ],
        series: vec![kb_series(101, "Wumbo Quest", Some(2015))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let series: Vec<_> = catalog
        .tv_series
        .iter()
        .map(|series| {
            let discs: Vec<_> = series.discs.iter().map(|id| id.0).collect();
            (series.tmdb_id, discs)
        })
        .collect();
    assert_eq!(series, vec![(Some(101), vec![1, 2, 3, 4, 5])]);
    for n in 21..=23 {
        let (_, log) = catalog.audit_logs.iter().find(|(id, _)| id.0 == n).unwrap();
        assert!(
            !log.claims
                .iter()
                .any(|claim| matches!(&claim.pred, ClaimPredicate::MovieTitle(t) if t.starts_with("disc"))),
            "{:?}",
            log.claims
        );
    }
    assert_eq!(
        movies_by_id(&catalog),
        vec![
            // Unknown to the knowledge base, and looked up by no disc's
            // number.
            (
                None,
                vec![
                    (21, Some(FileRole::Main)),
                    (22, Some(FileRole::Main)),
                    (23, Some(FileRole::Main))
                ]
            ),
            (Some(201), vec![(11, Some(FileRole::Main))]),
            (Some(202), vec![(12, Some(FileRole::Main))]),
        ]
    );
}

#[test]
fn a_disc_whose_folder_is_its_label_is_named_by_the_folder_its_in() {
    let files = [
        disc(
            1,
            "[BDMV] Zibble Lark [Vol.1~6Fin][JP]/BDROM/BDMV/index.bdmv",
            DiscKind::BluRay,
            "[BDMV] Zibble Lark [Vol.1~6Fin][JP]/BDROM",
            25.0,
        ),
        disc(
            2,
            "[BDMV] Glimmer Peaks/GLIMMER PEAKS VOL1/BDMV/index.bdmv",
            DiscKind::BluRay,
            "[BDMV] Glimmer Peaks/GLIMMER PEAKS VOL1",
            25.0,
        ),
    ];
    let claims = run_heuristics(&files);
    let titles = |subject: usize| -> Vec<String> {
        claims[&subject]
            .iter()
            .filter_map(|claim| match &claim.pred {
                ClaimPredicate::MovieTitle(title) => Some(title.clone()),
                _ => None,
            })
            .collect()
    };
    // Not the label's, which names nothing.
    assert_eq!(titles(0), vec!["zibble lark".to_string()]);
    // The folder it's in first, and its own, a volume's label, after.
    assert_eq!(
        titles(1),
        vec![
            "glimmer peaks".to_string(),
            "glimmer peaks vol1".to_string()
        ]
    );
}
