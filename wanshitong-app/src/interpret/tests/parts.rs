//! Parts of a work, e.g. `Part 2`, as a movie's or a series' episodes.

use super::*;

#[test]
fn parts_numbered_in_tags_are_looked_up_by_their_runtime_together() {
    let haruhi = "[Zork-Glorp] Suzumiya Haruhi no Yuuutsu";
    let gensou = format!("{haruhi}/[Zork-Glorp] Suzumiya Haruhi no Gensou [Wump_1080p]");
    let gekisou = format!("{haruhi}/[Zork-Glorp] Suzumiya Haruhi no Gekisou [Wump_480p]");
    let files = [
        video(
            1,
            &format!(
                "{gensou}/[Zork-Glorp] Suzumiya Haruhi no Gensou [01][Wump_1080p][x265_flac].mkv"
            ),
            52.7,
        ),
        video(
            2,
            &format!(
                "{gensou}/[Zork-Glorp] Suzumiya Haruhi no Gensou [02][Wump_1080p][x265_flac].mkv"
            ),
            57.35,
        ),
        video(
            3,
            &format!(
                "{gekisou}/[Zork-Glorp] Suzumiya Haruhi no Gekisou [01][Wump_480p][x265_flac].mkv"
            ),
            48.2,
        ),
        video(
            4,
            &format!(
                "{gekisou}/[Zork-Glorp] Suzumiya Haruhi no Gekisou [02][Wump_480p][x265_flac].mkv"
            ),
            65.7,
        ),
        // Not of a set: named otherwise, or numbered alone.
        video(
            5,
            &format!(
                "{gekisou}/SPs/[Zork-Glorp] Suzumiya Haruhi no Gekisou [Event][Wump_480p][x265_aac].mkv"
            ),
            81.1,
        ),
        video(6, "[Group] Some Movie [01][1080p].mkv", 100.0),
    ];
    let mut claims = run_heuristics(&files);
    deactivate_duplicate_claims(&mut claims);

    let parts: Vec<(i64, u32)> = (0..files.len())
        .flat_map(|subject| {
            let row_id = files[subject].1.row_id.0;
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter_map(move |claim| match claim.pred {
                    ClaimPredicate::Part(part) => Some((row_id, part)),
                    _ => None,
                })
        })
        .collect();
    assert_eq!(parts, vec![(1, 1), (2, 2), (3, 1), (4, 2)]);

    let runtimes = parts_runtimes(&files, &claims);
    let minutes = |subject: SubjectIndex| runtimes.get(&subject).map(|d| d.as_secs() / 60);
    // Each part's lookup matches the concert by both halves' runtime.
    assert_eq!(minutes(0), Some(110));
    assert_eq!(minutes(1), Some(110));
    assert_eq!(minutes(2), Some(113));
    assert_eq!(minutes(4), None);
    assert_eq!(minutes(5), None);
}

#[test]
fn a_file_named_as_a_part_of_its_work_says_which() {
    let files = [
        video(
            1,
            "The.Sorrow.and.the.Pity.1969.Part.One.The.Collapse.mkv",
            126.9,
        ),
        video(2, "Dune.Part.Two.2024.2160p.UHD.BluRay.REMUX.mkv", 166.0),
        video(3, "Back.to.the.Future.Part.II.1989.1080p.mkv", 108.0),
        video(4, "Once Upon a Time in America Pt.1.mkv", 115.0),
        // No parts here.
        video(5, "Party.Monster.2003.mkv", 98.0),
        video(6, "Departures.2008.mkv", 130.0),
        video(7, "Kill.Bill.Vol.2.2004.mkv", 137.0),
    ];
    let claims = run_heuristics(&files);

    let parts: Vec<(i64, u32)> = (0..files.len())
        .flat_map(|subject| {
            let row_id = files[subject].1.row_id.0;
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter_map(move |claim| match claim.pred {
                    ClaimPredicate::Part(part) => Some((row_id, part)),
                    _ => None,
                })
        })
        .collect();
    assert_eq!(parts, vec![(1, 1), (2, 2), (3, 2), (4, 1)]);
}

#[tokio::test]
async fn parts_are_episodes_of_a_series_where_theres_no_movie() {
    let sorrow = "The.Sorrow.and.the.Pity.1969.GBR.BluRay.Remux.1080p.AVC.DTS-HD.MA.2.0-Baz";
    let files = [
        // The directory takes one for its movie's main file.
        video(
            1,
            &format!(
                "{sorrow}/The.Sorrow.and.the.Pity.1969.Part.One.The.Collapse.GBR.BluRay.Remux.1080p.AVC.DTS-HD.MA.2.0-Baz.mkv"
            ),
            126.9,
        ),
        video(
            2,
            &format!(
                "{sorrow}/The.Sorrow.and.the.Pity.1969.Part.Two.The.Choice.GBR.BluRay.Remux.1080p.AVC.DTS-HD.MA.2.0-Baz.mkv"
            ),
            133.1,
        ),
        // A sequel, which is a movie.
        video(3, "Dune.Part.Two.2024.2160p.UHD.BluRay.REMUX.mkv", 166.0),
        // Titled before the part as TMDB titles the series.
        video(
            4,
            "Taskmaster.S01-S16+Specials.1080p.Web.AAC.HEVC.x265-Foobar/Season 1/Taskmaster.Champion.Of.Champions.Part.1.Wiley.Giraffe.Blower.1080p.Web.HEVC.x265-Foobar.mkv",
            44.9,
        ),
        video(
            5,
            "Taskmaster.S01-S16+Specials.1080p.Web.AAC.HEVC.x265-Foobar/Season 1/Taskmaster.Champion.Of.Champions.Part.2.Ive.Sinned.Again.1080p.Web.HEVC.x265-Foobar.mkv",
            44.6,
        ),
    ];
    let kb = CatalogKb {
        movies: vec![kb_movie(693134, "Dune: Part Two", Some(2024), Some(167))],
        series: vec![
            kb_series(128062, "The Sorrow and the Pity", None),
            kb_series(78840, "Taskmaster: Champion of Champions", None),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;

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
            (series.tmdb_id, episodes)
        })
        .collect();
    series.sort();
    assert_eq!(
        series,
        vec![
            (Some(78840), vec![(4, None, [1, 1]), (5, None, [2, 2])]),
            (Some(128062), vec![(1, None, [1, 1]), (2, None, [2, 2])]),
        ]
    );
    assert_eq!(
        movies_by_id(&catalog),
        vec![(Some(693134), vec![(3, Some(FileRole::Main))])]
    );
}
