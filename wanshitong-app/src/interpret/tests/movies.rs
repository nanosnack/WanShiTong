//! Movies: their titles, years, editions, IDs and main files.

use super::*;

#[test]
fn samples_are_excluded_from_the_catalog() {
    let files = [
        video(1, "Heat.1995.1080p-GRP/Heat.1995.1080p-GRP.mkv", 170.0),
        // In an extras-like directory, but a sample.
        video(2, "Heat.1995.1080p-GRP/Sample/heat.1995.1080p-grp.mkv", 1.0),
        video(
            3,
            "The.Last.of.the.Mohicans.1992-GRP/The.Last.of.the.Mohicans.1992-GRP.mkv",
            114.0,
        ),
        video(4, "The.Last.of.the.Mohicans.1992-GRP/Sample.mkv", 1.4),
        // The only video in its directory, so it's claimed as the main file.
        video(5, "Blade.1998-GRP/blade.1998-grp-sample.mkv", 0.5),
        // Named like a sample, but too long to be one.
        video(6, "The.Sample.2019.mkv", 95.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        movies(&catalog),
        vec![
            ("heat".into(), vec![(1, Some(FileRole::Main))]),
            (
                "the last of the mohicans".into(),
                vec![(3, Some(FileRole::Main))]
            ),
            ("the sample".into(), vec![(6, Some(FileRole::Main))]),
        ]
    );
    assert!(catalog.tv_series.is_empty());

    // The audit logs explain why the samples are missing.
    for sample in [2, 4, 5] {
        let active_roles: Vec<_> = active_claims(&catalog, sample)
            .into_iter()
            .filter_map(|pred| match pred {
                ClaimPredicate::FileRole(role) => Some(role),
                _ => None,
            })
            .collect();
        assert_eq!(active_roles, vec![FileRole::Sample], "file {sample}");
    }
}

#[test]
fn scans_are_not_movies() {
    // ffprobe reads a still image as one video stream.
    let scan = |row_id, rel_path, format: &str, codec: &str| {
        let (path_id, mut facts) = video(row_id, rel_path, 0.0);
        let media = facts.media.as_mut().unwrap();
        let container = media.container.as_mut().unwrap();
        container.format = format.into();
        container.duration_ms = None;
        media.video[0].codec = codec.into();
        (path_id, facts)
    };
    let haruhi = "[Zork-Glorp] Suzumiya Haruhi no Yuuutsu [Wump_1080p]";
    let files = [
        scan(
            1,
            &format!("{haruhi}/Scans/2006 BDBOX/BD3.webp"),
            "webp_pipe",
            "webp",
        ),
        scan(2, &format!("{haruhi}/Scans/0001.webp"), "image2", "webp"),
        scan(3, "Heat (1995)/Scans/Cover.tif", "tiff_pipe", "tiff"),
        video(4, "Heat (1995)/Heat (1995).mkv", 170.0),
        // Told by its MIME type, as its demuxer is its own.
        {
            let curb = "Curb Your Enthusiasm Season 1, 2, 3, 4, 5, 6, 7 & 8 + Extras DVDRip Zib";
            let (path_id, mut facts) = scan(
                5,
                &format!("{curb}/Curb Your Enthusiasm Icon.ico"),
                "ico",
                "bmp",
            );
            facts.mime_type = Some("image/x-icon".into());
            (path_id, facts)
        },
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        movies(&catalog),
        vec![("heat".into(), vec![(4, Some(FileRole::Main))])]
    );
    assert!(catalog.tv_series.is_empty());
}

#[test]
fn manifestations_of_a_movie_are_all_main_files() {
    let mut other_source = video(3, "Heat.1995.Remux.mkv", 170.0);
    other_source.0.data_source = DataSourceUuid(Uuid::from_u128(1));
    let files = [
        video(1, "Heat.1995.1080p.mkv", 170.0),
        video(2, "Heat (1995)/Heat (1995).mkv", 170.0),
        other_source,
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        movies(&catalog),
        vec![(
            "heat".into(),
            vec![
                (1, Some(FileRole::Main)),
                (2, Some(FileRole::Main)),
                (3, Some(FileRole::Main)),
            ]
        )]
    );
}

#[test]
fn each_copy_of_a_movie_realizes_the_edition_its_main_files_are() {
    let files = [
        video(1, "Alien.3.1992.Assembly.Cut.REMUX.mkv", 145.0),
        video(
            2,
            "Alien 3 (1992) 2160p/Alien.3.1992.Assembly.Cut.2160p.mkv",
            145.0,
        ),
        video(3, "Alien 3 (1992) 2160p/Extras/Making Of.mkv", 20.0),
        // Nothing says which cut it is.
        video(4, "Alien.3.1992.1080p.mkv", 114.0),
        // Its parts don't agree, so nothing says which cut it is either.
        video(
            5,
            "Alien 3 (1992) Parts/Alien.3.1992.Extended.CD1.mkv",
            70.0,
        ),
        video(6, "Alien 3 (1992) Parts/Alien.3.1992.CD2.mkv", 75.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let [movie] = &catalog.movies[..] else {
        panic!("one movie: {:?}", movies(&catalog));
    };
    assert_eq!(
        movie.expressions,
        vec![
            NewMovieExpression { edition: None },
            NewMovieExpression {
                edition: Some("Assembly Cut".into())
            },
        ]
    );
    let mut realizes: Vec<_> = movie
        .files
        .iter()
        .map(|file| (file.file_id.0, file.role, file.expression))
        .collect();
    realizes.sort();
    assert_eq!(
        realizes,
        vec![
            (1, Some(FileRole::Main), Some(1)),
            (2, Some(FileRole::Main), Some(1)),
            (3, Some(FileRole::Extra), None),
            (4, Some(FileRole::Main), Some(0)),
            (5, Some(FileRole::Main), Some(0)),
            (6, Some(FileRole::Main), Some(0)),
        ]
    );
}

#[test]
fn a_movie_without_evidence_of_its_main_file_has_none() {
    // Nothing says this is the feature rather than, say, a trailer.
    let files = [video(1, "heat.mkv", 170.0)];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(movies(&catalog), vec![("heat".into(), vec![(1, None)])]);
}

#[test]
fn files_that_share_any_id_are_one_movie() {
    let media = DataSourceUuid(Uuid::nil());
    let files = [
        video(1, "Heat.1995.mkv", 170.0),
        video(2, "Heat.1995.Remux.mkv", 170.0),
        video(3, "Heat (1995).mkv", 170.0),
        video(4, "Blade.1998.mkv", 120.0),
    ];
    let id = |id| ClaimPredicate::MovieExternalId(id);
    let assertions = [
        assertion(media, &["Heat.1995.mkv"], id(ExternalId::Tmdb(949))),
        assertion(
            media,
            &["Heat.1995.mkv", "Heat.1995.Remux.mkv"],
            id(ExternalId::Imdb("tt0113277".into())),
        ),
        // Shares nothing with the first file, only with the second.
        assertion(
            media,
            &["Heat.1995.Remux.mkv", "Heat (1995).mkv"],
            id(ExternalId::Tvdb(7)),
        ),
        assertion(media, &["Blade.1998.mkv"], id(ExternalId::Tmdb(36647))),
    ];
    let catalog = catalog_without_kb(&files, &assertions);

    assert_eq!(
        movies_by_id(&catalog),
        vec![
            (
                Some(949),
                vec![
                    (1, Some(FileRole::Main)),
                    (2, Some(FileRole::Main)),
                    (3, Some(FileRole::Main)),
                ]
            ),
            (Some(36647), vec![(4, Some(FileRole::Main))]),
        ]
    );
}

#[test]
fn container_title_tags_only_name_videos() {
    let with_title = |(path_id, mut facts): (PathId, FileFacts), title: &str| {
        let container = facts.media.as_mut().unwrap().container.as_mut().unwrap();
        container.tags = Some([("TITLE".into(), title.into())].into());
        (path_id, facts)
    };
    let mut track = with_title(video(1, "OST/03. Exist.flac", 3.9), "Exist");
    track.1.media.as_mut().unwrap().video.clear();
    let files = [track, with_title(video(2, "feature.mkv", 90.0), "Exist")];
    let catalog = catalog_without_kb(&files, &[]);

    // Nothing says the video is the feature, only which movie it is.
    assert_eq!(movies(&catalog), vec![("exist".into(), vec![(2, None)])]);
}

#[tokio::test]
async fn a_movie_is_described_by_one_knowledge_base_result() {
    let files = [video(1, "Exist.2013.mkv", 90.0)];
    let claim = Ranked::new;
    let kb = FixedKb(vec![
        // The best result, from a search that doesn't give IMDB IDs.
        vec![
            claim(20.0, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1))),
            claim(20.0, ClaimPredicate::MovieTitle("Ash 01".into())),
        ],
        vec![
            claim(10.0, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(2))),
            claim(
                10.0,
                ClaimPredicate::MovieExternalId(ExternalId::Imdb("tt2".into())),
            ),
            claim(10.0, ClaimPredicate::MovieTitle("Exist".into())),
            claim(10.0, ClaimPredicate::MovieReleaseYear(2013)),
        ],
    ]);
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    let movies: Vec<_> = catalog
        .movies
        .iter()
        .map(|movie| {
            (
                movie.title.as_str(),
                movie.tmdb_id,
                movie.imdb_id.as_deref(),
            )
        })
        .collect();
    // Not the other result's IMDB ID. The year still comes from the file.
    assert_eq!(movies, vec![("Ash 01", Some(1), None)]);
    assert_eq!(catalog.movies[0].release_year, Some(2013));
}

#[test]
fn a_title_is_also_looked_up_without_its_release_noise() {
    let pm = "Perry Mason/TV Movies (Complete)";
    let berserk = "[Frob] Berserk (1997) (1080p Blu-ray 10bit Dual Audio)";
    let mut files = vec![
        video(
            1,
            &format!("{pm}/M-01 Perry Mason - Perry Mason Returns.avi"),
            95.7,
        ),
        video(
            2,
            &format!("{pm}/M-13 Perry Mason - The Poisoned Pen  tv-xvid.avi"),
            87.8,
        ),
        video(
            3,
            "Tokyo Godfathers The Ultimate Enthusiast's Collected Edition/Tokyo Godfathers USBD Remux.mkv",
            92.9,
        ),
        video(
            4,
            "(アニメ)(映画)東映まんがまつり グリム童話 金の鳥(1987) (東映チャンネルHD 1440x1080 x265).mp4",
            52.1,
        ),
        // What's left would be a word, which a title's noise is too.
        video(5, "Japanese Blu-ray Promos.mkv", 12.0),
        // Already without noise, before its year.
        video(6, "Charlotte's Web 2006 1080p WEB-DL.mkv", 97.0),
        // A number starting a title isn't numbering.
        video(7, "12 Angry Men 1080p BluRay.mkv", 96.0),
        // Words of a streaming release, and `Max` in a title but `MAX` not.
        video(8, "Mad.Max.Fury.Road.MAX.WEB-DL.DDP5.1.Atmos.mkv", 120.0),
        // A release's words in brackets, with others of its before them.
        video(9, "Wumbo Lark Live (FNORD 1280x720 x264 AAC).mkv", 98.0),
        video(10, "Glimmer Peaks {FR 720p}{JPN AAC}.mkv", 103.0),
        // A fansub's words: frame size, bit depth, subtitles' language.
        video(21, "Zibble Road - Fhd 8 bits VOSTFR.mkv", 24.0),
        video(22, "Yonder Fnord BD Sub ITA.mkv", 24.0),
        video(23, "Wumbo Lark (Blorp 1440x1080 Hi10P).mkv", 24.0),
        // A title that begins with a release's word, cut at the next.
        video(24, "8-bit Glimmer 1080p.mkv", 90.0),
        // An edition, or a language, against the release's words or year.
        video(25, "Zibble.Road.Unrated.2006.1080p.BluRay.mkv", 101.0),
        video(26, "Yonder.Peak.Spanish.1080p.WEB-DL.mkv", 92.0),
        // A word of the title's, though a language too.
        video(27, "The.Spanish.Wumbo.2001.1080p.mkv", 95.0),
    ];
    // Episodes, numbered or named so, have no movie title.
    for n in 1..=3 {
        files.push(video(
            10 + n,
            &format!("{berserk}/Berserk 0{n} (1080p Blu-ray 10bit Dual Audio).mkv"),
            24.0,
        ));
    }
    files.push(video(
        20,
        "True Detective/True.Detective.S01E01.1080p.BluRay.x264.mkv",
        58.0,
    ));
    let claims = run_heuristics(&files);
    let files = &files;

    let titles: Vec<(i64, &str)> = (0..files.len())
        .flat_map(|subject| {
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter_map(move |claim| match (&claim.source, &claim.pred) {
                    (
                        ClaimSource::Heuristic(Heuristic::TitleWithoutReleaseNoise),
                        ClaimPredicate::MovieTitle(title),
                    ) => Some((files[subject].1.row_id.0, title.as_str())),
                    _ => None,
                })
        })
        .collect();
    assert_eq!(
        titles,
        vec![
            (1, "perry mason - perry mason returns"),
            (2, "perry mason - the poisoned pen"),
            (3, "tokyo godfathers"),
            (4, "東映まんがまつり グリム童話 金の鳥"),
            (7, "12 angry men"),
            (8, "mad max fury road"),
            (9, "wumbo lark live"),
            (10, "glimmer peaks"),
            (21, "zibble road -"),
            (22, "yonder fnord"),
            (23, "wumbo lark"),
            (24, "8-bit glimmer"),
            (25, "zibble road"),
            (26, "yonder peak"),
        ]
    );
}

#[test]
fn a_movies_year_is_the_one_after_its_title() {
    let files = [
        // A title's number yet to come as a year.
        video(1, "Blade.Runner.2049.2017.1080p.BluRay.mkv", 164.0),
        // A title that begins with one.
        video(2, "2001.A.Space.Odyssey.1968.2160p.mkv", 149.0),
        video(3, "1917.2019.UHD.BluRay.mkv", 119.0),
        // A title's year-like number, before the year, and a range's years.
        video(4, "Wonder.Woman.1984.2020.1080p.BluRay.mkv", 151.0),
        video(5, "Berserk Golden Age Arc (2012-2013).mkv", 77.0),
        // Not a word of its own.
        video(6, "99.Homes.2015.1080p.WEB-DL.x264-Blorp2020.mkv", 112.0),
        // A width, not a year.
        video(7, "Wumbo Live (BD 1920x1080 x264 FLAC).mkv", 112.0),
        // The year twice.
        video(8, "Wumbo.Lark.2025.2025.1080p.WEB-DL.mkv", 101.0),
        // In square brackets, alone: not a tag.
        video(9, "Zibble.[2002].1080p.WEB-DL.[Blorp].mkv", 98.0),
    ];
    let claims = run_heuristics(&files);
    let files = &files;

    let parsed: Vec<(i64, String, Option<u32>)> = (0..files.len())
        .map(|subject| {
            let claims: Vec<&ClaimPredicate> = claims[&subject]
                .iter()
                .filter(|claim| claim.source == ClaimSource::Heuristic(Heuristic::FileNameTitle))
                .map(|claim| &claim.pred)
                .collect();
            let title = claims.iter().find_map(|pred| match pred {
                ClaimPredicate::MovieTitle(title) => Some(title.clone()),
                _ => None,
            });
            let year = claims.iter().find_map(|pred| match pred {
                ClaimPredicate::MovieReleaseYear(year) => Some(*year),
                _ => None,
            });
            (files[subject].1.row_id.0, title.unwrap(), year)
        })
        .collect();
    assert_eq!(
        parsed,
        vec![
            (1, "blade runner 2049".into(), Some(2017)),
            (2, "2001 a space odyssey".into(), Some(1968)),
            (3, "1917".into(), Some(2019)),
            (4, "wonder woman 1984".into(), Some(2020)),
            (5, "berserk golden age arc".into(), Some(2012)),
            (6, "99 homes".into(), Some(2015)),
            (7, "wumbo live (bd 1920x1080 x264 flac)".into(), None),
            (8, "wumbo lark".into(), Some(2025)),
            (9, "zibble".into(), Some(2002)),
        ]
    );
}

#[test]
fn ids_written_in_names_are_claimed() {
    let op = "One Piece (1999) [tvdbid-81797]";
    let files = [
        video(
            1,
            "Alien³ (1992) {tmdb-8077} {edition-Assembly Cut} [Bluray-2160p]-Zot.mkv",
            145.0,
        ),
        video(2, "Heat (1995) [imdbid-tt0113277]/Heat.mkv", 170.0),
        // An episode, by its marker, and of a run of its siblings.
        video(
            3,
            &format!("{op}/Arc 01 - Romance Dawn/One.Piece.E1.mkv"),
            24.0,
        ),
        video(4, "Berserk (1997) [tvdbid-73752]/Berserk 01.mkv", 24.0),
        video(5, "Berserk (1997) [tvdbid-73752]/Berserk 02.mkv", 24.0),
        video(6, "Berserk (1997) [tvdbid-73752]/Berserk 03.mkv", 24.0),
        // A movie two directories below a series' ID.
        video(
            7,
            &format!("{op}/Movies/One Piece Film Red (2022).mkv"),
            115.0,
        ),
        // An extra's, which is of its main file's work.
        video(8, "Heat (1995) [imdbid-tt0113277]/Extras/Trailer.mkv", 2.0),
        // Not an ID.
        video(
            9,
            "One Punch Man (2015) [tbdbid-293088]/One Punch Man - S01E01.mkv",
            24.0,
        ),
    ];
    let claims = run_heuristics(&files);
    let files = &files;

    let ids: Vec<(i64, ClaimPredicate)> = (0..files.len())
        .flat_map(|subject| {
            claims
                .get(&subject)
                .into_iter()
                .flatten()
                .filter(|claim| claim.source == ClaimSource::Heuristic(Heuristic::NameExternalId))
                .map(move |claim| (files[subject].1.row_id.0, claim.pred.clone()))
        })
        .collect();
    assert_eq!(
        ids,
        vec![
            (1, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(8077))),
            (
                2,
                ClaimPredicate::MovieExternalId(ExternalId::Imdb("tt0113277".into()))
            ),
            (
                3,
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tvdb(81797))
            ),
            (
                4,
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tvdb(73752))
            ),
            (
                5,
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tvdb(73752))
            ),
            (
                6,
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tvdb(73752))
            ),
        ]
    );
}

#[test]
fn a_file_name_says_which_edition_it_is() {
    let files = [
        video(
            1,
            "A.Knights.Tale.2001.Extended.2160p.UHD.Blu-ray.Remux.mkv",
            144.0,
        ),
        video(
            2,
            "Brazil (1985) DC Criterion (1080p BluRay x265 10bit Zonk).mkv",
            142.0,
        ),
        video(
            3,
            "Kingdom.of.Heaven.DC.Roadshow.Version.2005.2160p.UHD.Blu-ray.Remux.mkv",
            194.0,
        ),
        video(
            4,
            "Alien.3.Assembly.Cut.1992.BluRay.1080p.DTS-HD.MA.5.1.AVC.REMUX-Grault.mkv",
            145.0,
        ),
        video(
            5,
            "Once.Upon.a.Time.in.America.1984.Extended.Director's.Cut.BluRay.1080p.mkv",
            251.0,
        ),
        video(
            6,
            "Alien³ (1992) {tmdb-8077} {edition-Assembly Cut} [Bluray-2160p]-Zot.mkv",
            145.0,
        ),
        // Titles that start with an edition's words.
        video(7, "Uncut.Gems.2019.2160p.BluRay.REMUX.mkv", 135.0),
        video(8, "The.Final.Cut.2004.1080p.BluRay.mkv", 95.0),
        // `dc` in a title.
        video(9, "Washington.dc.Story.2010.1080p.mkv", 90.0),
        // A four-byte word that isn't a year, e.g. with a two-byte letter.
        video(12, "El Dia de aña 2001 Extended.mkv", 100.0),
        // An extra isn't a cut of its work.
        video(10, "Brazil (1985)/Extras/Theatrical Trailer.mkv", 2.0),
        video(
            11,
            "A Knights Tale (2001)/Featurettes/Deleted Scenes - Extended (2001).mkv",
            12.0,
        ),
    ];
    let claims = run_heuristics(&files);
    let files = &files;
    let claimed = |heuristic: Heuristic| -> Vec<(i64, String)> {
        (0..files.len())
            .flat_map(|subject| {
                claims
                    .get(&subject)
                    .into_iter()
                    .flatten()
                    .filter(move |claim| claim.source == ClaimSource::Heuristic(heuristic))
                    .filter_map(move |claim| match &claim.pred {
                        ClaimPredicate::Edition(edition) | ClaimPredicate::MovieTitle(edition) => {
                            Some((files[subject].1.row_id.0, edition.clone()))
                        }
                        _ => None,
                    })
            })
            .collect()
    };

    assert_eq!(
        claimed(Heuristic::FileNameEdition),
        vec![
            (1, "Extended".into()),
            (2, "Director's Cut".into()),
            (3, "Director's Cut Roadshow".into()),
            (4, "Assembly Cut".into()),
            (5, "Extended Director's Cut".into()),
            (6, "Assembly Cut".into()),
            (12, "Extended".into()),
        ]
    );
    // An edition before the year would be looked up as the title's.
    assert_eq!(
        claimed(Heuristic::TitleWithoutEdition),
        vec![(3, "kingdom of heaven".into()), (4, "alien 3".into())]
    );
}

#[test]
fn a_series_movie_is_also_looked_up_without_its_number() {
    let files = [
        video(
            1,
            "[Foo] Code Geass Movie 01 - Lelouch of the Rebellion - Initiation.mkv",
            135.0,
        ),
        video(
            2,
            "[Foo] Code Geass OVA04 - Akito The Exiled - 03.mkv",
            60.0,
        ),
        // The number is part of these titles.
        video(3, "Scary Movie 2 (2001).mkv", 83.0),
        video(4, "Movie 43 (2013).mkv", 94.0),
    ];
    let claims = run_heuristics(&files);
    let markers = ReleaseMarkers::new();
    let titles = |subject: SubjectIndex| -> Vec<String> {
        let mut titles: Vec<_> = find_kb_search_fields(&claims[&subject], |t| markers.in_title(t))
            .unwrap()
            .titles
            .into_iter()
            .map(|title| title.value)
            .collect();
        titles.sort();
        titles
    };

    assert_eq!(
        titles(0),
        vec![
            "code geass - lelouch of the rebellion - initiation",
            "code geass movie 01 - lelouch of the rebellion - initiation",
        ]
    );
    assert_eq!(
        titles(1),
        vec![
            "code geass - akito the exiled - 03",
            "code geass ova04 - akito the exiled - 03",
        ]
    );
    // Both are looked up, but the file name's title outranks the other.
    assert_eq!(titles(2), vec!["scary", "scary movie 2"]);
    assert_eq!(titles(3), vec!["movie 43"]);

    let catalog = catalog_without_kb(&files, &[]);
    let names: Vec<_> = movies(&catalog)
        .into_iter()
        .map(|(title, _)| title)
        .collect();
    assert!(names.contains(&"scary movie 2".to_string()));
}

/// TMDB's entry for a cut of a movie names the edition, but the file's name
/// says which cut the file is, and may say more. Whatever found the movie:
/// here its directory, which outranks the file name's edition.
#[tokio::test]
async fn the_file_name_s_edition_outranks_the_knowledge_base_s() {
    let files = [video(
        1,
        "Kingdom of Heaven (2005)/Kingdom.of.Heaven.DC.Roadshow.Version.2005.2160p.UHD.Blu-ray.Remux.mkv",
        194.0,
    )];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(1495, "Kingdom of Heaven", Some(2005), Some(144)),
            kb_movie(
                1759035,
                "Kingdom Of Heaven (Director's Cut)",
                Some(2005),
                Some(194),
            ),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;

    assert_eq!(
        movies(&catalog),
        vec![("Kingdom of Heaven".into(), vec![(1, Some(FileRole::Main))])]
    );
    let (_, log) = catalog.audit_logs.iter().find(|(id, _)| id.0 == 1).unwrap();
    let editions: Vec<_> = log
        .claims
        .iter()
        .filter_map(|claim| match &claim.pred {
            ClaimPredicate::Edition(edition) => Some((edition.as_str(), claim.active)),
            _ => None,
        })
        .collect();
    assert_eq!(
        editions,
        [("Director's Cut Roadshow", true), ("Director's Cut", false)]
    );
}

/// Each main file's row ID, manifestation and part, sorted by row ID.
fn manifestations(catalog: &NewCatalog) -> Vec<(i64, Option<usize>, Option<u32>)> {
    let mut manifestations: Vec<_> = catalog
        .movies
        .iter()
        .flat_map(|movie| &movie.files)
        .filter(|file| file.role == Some(FileRole::Main))
        .map(|file| (file.file_id.0, file.manifestation, file.part))
        .collect();
    manifestations.sort();
    manifestations
}

#[test]
fn main_files_beside_each_other_are_copies_of_their_own() {
    // Two encodes of one movie in one directory. Nothing numbers them as
    // parts of one manifestation, so they're two, and a media server shows both.
    let files = [
        video(1, "Heat (1995)/Heat.1995.1080p.mkv", 170.0),
        video(2, "Heat (1995)/Heat.1995.720p.mkv", 170.0),
        video(3, "Heat.1995.Remux.mkv", 170.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        movies(&catalog),
        vec![(
            "heat".into(),
            vec![
                (1, Some(FileRole::Main)),
                (2, Some(FileRole::Main)),
                (3, Some(FileRole::Main)),
            ]
        )]
    );
    assert_eq!(
        manifestations(&catalog),
        vec![(1, Some(0), None), (2, Some(1), None), (3, Some(2), None)]
    );
}

#[test]
fn parts_beside_each_other_are_one_copy() {
    let files = [
        video(1, "Heat (1995)/Heat.1995.CD2.mkv", 85.0),
        video(2, "Heat (1995)/Heat.1995.CD1.mkv", 85.0),
        // The same parts elsewhere are another manifestation.
        video(3, "Heat 1995 DVD/Heat.1995.Disc 1.mkv", 85.0),
        video(4, "Heat 1995 DVD/Heat.1995.Disc 2.mkv", 85.0),
        // A DVD's size, not a part.
        video(5, "Heat.1995.DVD5.mkv", 170.0),
        // A part on its own is a manifestation of one file, so no part of it
        // is recorded, as a media server would look for the others.
        video(6, "Heat 1995 Lone/Heat.1995.CD1.mkv", 170.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    assert_eq!(
        manifestations(&catalog),
        vec![
            (1, Some(0), Some(2)),
            (2, Some(0), Some(1)),
            (3, Some(1), Some(1)),
            (4, Some(1), Some(2)),
            (5, Some(3), None),
            (6, Some(2), None),
        ]
    );
}

/// A movie's directory is one wherever it is, not only at a data source's
/// top: under a grouping directory, its name with a year says so, and so
/// does a name that's its video's. A directory that nothing names so, and
/// has no extras directory, isn't a movie's, though it holds one video.
#[test]
fn a_movie_directory_is_found_at_any_depth_by_its_name() {
    let files = [
        video(1, "Movies/Alien (1979)/Alien.mkv", 117.0),
        video(2, "Movies/Sci-Fi/Aliens/Aliens.mkv", 137.0),
        // A release group's name, which names no movie.
        video(3, "Movies/Some.Release.Group.Pack/the.thing.mkv", 109.0),
        // Short, so not a second feature: the directory is still one movie's.
        video(4, "Movies/Alien (1979)/Alien - Trailer.mkv", 2.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let title_of = |file_id| {
        active_claims(&catalog, file_id)
            .into_iter()
            .find_map(|claim| match claim {
                ClaimPredicate::MovieTitle(title) => Some(title),
                _ => None,
            })
    };
    assert_eq!(title_of(1), Some("alien".into()));
    assert_eq!(title_of(2), Some("aliens".into()));
    assert_eq!(title_of(3), Some("the thing".into()));
    let mut found = movies(&catalog);
    // Nothing names the trailer as an extra yet, so it's a movie of its own.
    found.retain(|(title, _)| title != "alien - trailer");
    assert_eq!(
        found,
        vec![
            ("alien".into(), vec![(1, Some(FileRole::Main))]),
            ("aliens".into(), vec![(2, Some(FileRole::Main))]),
            // Nothing says which of its files is the feature.
            ("the thing".into(), vec![(3, None)]),
        ]
    );
}

/// A directory of several features isn't a movie's, whatever its name says:
/// a box set, or a collection of TV movies. Its videos are movies by their
/// own names. A directory of a movie's parts, e.g. `CD1` and `CD2`, is one
/// movie's, and each part is a main file.
#[test]
fn a_directory_of_several_features_is_no_movies() {
    let files = [
        video(1, "Alien Quadrilogy (1979-1997)/Alien.1979.mkv", 117.0),
        video(2, "Alien Quadrilogy (1979-1997)/Aliens.1986.mkv", 137.0),
        video(3, "Alien Quadrilogy (1979-1997)/Alien.3.1992.mkv", 114.0),
        video(
            4,
            "Collections/Perry Mason/TV Movies (Complete)/M-01 Perry Mason - Perry Mason Returns.avi",
            95.0,
        ),
        video(
            5,
            "Collections/Perry Mason/TV Movies (Complete)/M-02 Perry Mason - The Case of the Notorious Nun.avi",
            95.0,
        ),
        video(6, "Heat (1995)/Heat.CD1.mkv", 85.0),
        video(7, "Heat (1995)/Heat.CD2.mkv", 85.0),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let titles: Vec<String> = catalog
        .movies
        .iter()
        .map(|movie| movie.title.clone())
        .collect();
    assert!(
        !titles
            .iter()
            .any(|title| title.contains("quadrilogy") || title.contains("tv movies")),
        "{titles:?}"
    );
    let heat: Vec<_> = movies(&catalog)
        .into_iter()
        .filter(|(title, _)| title == "heat")
        .collect();
    assert_eq!(
        heat,
        vec![(
            "heat".into(),
            vec![(6, Some(FileRole::Main)), (7, Some(FileRole::Main))]
        )]
    );
}

/// A directory named for what it holds, e.g. `Special`, names no movie, even
/// when its video's name begins the same way: Chibi Maruko-chan's
/// `Special/Special (1991) - Maruko's Wasted Summer` is the series' special,
/// which the specials heuristics say, not a movie called "special".
#[test]
fn a_directory_named_for_a_kind_of_video_is_no_movies() {
    let chibi = "[MaruChanSubs] Chibi Maruko-chan";
    let files = [
        video(1, &format!("{chibi}/01.MP4"), 21.2),
        video(2, &format!("{chibi}/02.MP4"), 21.2),
        video(3, &format!("{chibi}/03.MP4"), 21.2),
        video(
            4,
            &format!("{chibi}/Special/Special (1991) - Maruko's Wasted Summer.mp4"),
            35.3,
        ),
    ];
    let catalog = catalog_without_kb(&files, &[]);

    let (_, log) = catalog.audit_logs.iter().find(|(id, _)| id.0 == 4).unwrap();
    let from_directory: Vec<_> = log
        .claims
        .iter()
        .filter(|claim| claim.source == ClaimSource::Heuristic(Heuristic::MovieDirectory))
        .map(|claim| claim.pred.clone())
        .collect();
    assert!(from_directory.is_empty(), "{from_directory:?}");
    assert!(
        active_claims(&catalog, 4).contains(&ClaimPredicate::SpecialOf("chibi maruko-chan".into()))
    );
}

/// A set of movies numbered like episodes, e.g. Star Wars', is still searched
/// for by its titles: `Episode 1` is the title's, not a release's marker.
#[test]
fn a_movie_numbered_like_an_episode_keeps_its_title_for_the_search() {
    let files = [
        video(
            1,
            "Star Wars/Star Wars - Episode 1 - The Phantom Menace (1999).mkv",
            136.0,
        ),
        video(
            2,
            "Star Wars/Star Wars - Episode 2 - Attack of the Clones (2002).mkv",
            142.0,
        ),
    ];
    let claims = run_heuristics(&files);
    let markers = ReleaseMarkers::new();

    let fields = find_kb_search_fields(&claims[&0], |t| markers.in_title(t)).unwrap();

    assert!(fields.is_episode());
    let titles: Vec<_> = fields.titles.iter().map(|t| t.value.as_str()).collect();
    assert!(
        titles.contains(&"star wars - episode 1 - the phantom menace"),
        "{titles:?}"
    );
}

#[test]
fn a_movie_named_only_in_tags_is_titled_by_the_tag_after_the_groups() {
    let files = [
        // The release's tags around it, before and after.
        video(
            1,
            "[Blorp-Raws][4K][Wumbo Divine Flame][2160P][BDRemux][DTS-HDMA].mkv",
            98.0,
        ),
        // A year in it.
        video(
            2,
            "[Fnord][Zibble 1992][GB][BDRip][2072P][HEVC_AAC][MP4].mp4",
            94.0,
        ),
        // A year in a tag of its own, and a release's word before.
        video(
            3,
            "Glimmer/[Fnord][RAW][The Glimmer][2019][1080P].mkv",
            101.0,
        ),
        // A number that's no year: an episode or a part, not a movie's title.
        video(4, "Yonder/[Fnord][Yonder Lark][06][1080p][JPSC].mp4", 24.0),
        // An extra's tags name its series.
        video(5, "Yonder/[Fnord][Yonder Lark][NCED1][1080p].mkv", 2.0),
    ];
    let claims = run_heuristics(&files);
    let title_claims = |subject: usize| -> Vec<ClaimPredicate> {
        claims
            .get(&subject)
            .into_iter()
            .flatten()
            .filter(|claim| claim.source == ClaimSource::Heuristic(Heuristic::FileNameTitle))
            .map(|claim| claim.pred.clone())
            .collect()
    };
    assert_eq!(
        title_claims(0),
        vec![ClaimPredicate::MovieTitle("wumbo divine flame".into())]
    );
    assert_eq!(
        title_claims(1),
        vec![
            ClaimPredicate::MovieTitle("zibble".into()),
            ClaimPredicate::MovieReleaseYear(1992),
        ]
    );
    assert_eq!(
        title_claims(2),
        vec![
            ClaimPredicate::MovieTitle("the glimmer".into()),
            ClaimPredicate::MovieReleaseYear(2019),
        ]
    );
    assert_eq!(title_claims(3), vec![]);
    assert_eq!(title_claims(4), vec![]);
}

#[tokio::test]
async fn a_title_that_finds_nothing_is_looked_up_without_its_subtitle() {
    let files = [
        // The series named as its title before the subtitle begins, not its
        // movie titled so too.
        video(1, "Shows/Glimmer Peaks ~ The Long Night.mkv", 25.0),
        // A movie titled as it is.
        video(2, "Movies/Yonder Fnord ~ The Bright Road (2004).mkv", 100.0),
        // A work found by the words before the subtitle, but titled
        // otherwise: not the file's.
        video(3, "Movies/Fnord Glimmer ~ Night.mkv", 100.0),
        // One Latin word is too little to look up.
        video(4, "Movies/Wumbo ~ Zibble Night.mkv", 100.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(101, "Glimmer Peaks: Final Stage", Some(2019), None),
            kb_movie(102, "Yonder Fnord", Some(2004), None),
            kb_movie(103, "The Fnord Glimmer", Some(2004), None),
            kb_movie(104, "Wumbo", Some(2011), None),
        ],
        series: vec![kb_series(301, "Glimmer Peaks: Wanderers", Some(2018))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(None, 3), (None, 4), (Some(102), 2)]);
    let series: Vec<(Option<u32>, Vec<i64>)> = catalog
        .tv_series
        .iter()
        .map(|series| (series.tmdb_id, series.file_ids().map(|id| id.0).collect()))
        .collect();
    assert_eq!(series, vec![(Some(301), vec![1])]);
}

#[tokio::test]
async fn a_title_in_camel_case_that_finds_nothing_is_looked_up_split() {
    let files = [
        // Found by its words split at their capitals.
        video(1, "Movies/YonderFnordTales.mkv", 100.0),
        // A name written so of its own, which finds its work as it is.
        video(2, "Movies/WumboZibble.mkv", 100.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(201, "Yonder Fnord Tales", Some(2010), None),
            kb_movie(202, "WumboZibble", Some(2012), None),
            kb_movie(203, "Wumbo Zibble", Some(2014), None),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let found: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(found, vec![(Some(201), 1), (Some(202), 2)]);
}

#[tokio::test]
async fn a_scene_name_is_looked_up_without_its_groups_tag() {
    let files = [
        video(1, "zorp-glimmer.peaks.2019.720p.bluray.x264.mkv", 100.0),
        video(2, "Shows/fnordo-wumbo.lark.s01e02.1080p.web.mkv", 45.0),
        // A title whose first letter is joined to the rest.
        video(3, "z-blorp.2011.1080p.mkv", 100.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(801, "Glimmer Peaks", Some(2019), None),
            kb_movie(803, "Blorp", Some(2011), None),
        ],
        series: vec![kb_series(802, "Wumbo Lark", Some(2020))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(None, 3), (Some(801), 1)]);
    let series: Vec<Option<u32>> = catalog.tv_series.iter().map(|s| s.tmdb_id).collect();
    assert_eq!(series, vec![Some(802)]);
}

#[tokio::test]
async fn a_last_resort_movie_is_one_of_the_files_year() {
    let files = [
        // Titled so, but another year's: another work, e.g. a remake.
        video(1, "Movies/Zibble Road ~ Night (2010).mkv", 100.0),
        // Of its year: the movie, over a series named alike.
        video(2, "Movies/Glorp Tide ~ Dawn (2012).mkv", 100.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(901, "Zibble Road", Some(1950), None),
            kb_movie(902, "Glorp Tide", Some(2012), None),
        ],
        series: vec![kb_series(903, "Glorp Tide", Some(2015))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(None, 1), (Some(902), 2)]);
    assert!(catalog.tv_series.is_empty());
}

#[tokio::test]
async fn a_title_is_looked_up_without_and_written_for_an_ampersand() {
    let files = [
        video(1, "Movies/Zibble.and.Wumbos.Big.Night.2007.1080p.mkv", 80.0),
        // `plus` for `+`.
        video(2, "Movies/Glimmer.plus.Fnord.2025.720p.mkv", 95.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(961, "Zibble & Wumbo's Big Night", Some(2007), None),
            kb_movie(962, "Glimmer+Fnord", Some(2025), None),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(Some(961), 1), (Some(962), 2)]);
}

#[tokio::test]
async fn a_title_of_one_word_without_release_words_is_a_last_resort() {
    let files = [
        // A movie, not a series named so.
        video(1, "Movies/Zibble 1080p AMZN WEBDL x264 DDP 5 1.mkv", 120.0),
        // A word a release adds, left of a name it then names nothing of.
        video(2, "Movies/Japanese Blu-ray Promos.mkv", 12.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(971, "Zibble", Some(2002), None),
            kb_movie(972, "Japanese", Some(2011), None),
        ],
        series: vec![kb_series(973, "Zibble", Some(2019))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(None, 2), (Some(971), 1)]);
}

#[tokio::test]
async fn a_title_spelled_nearly_is_found_by_its_other_words_and_its_year() {
    let files = [
        video(1, "Movies/The.Zibble.Nights.2025.1080p.WEB-DL.mkv", 120.0),
        // Spelled nearly, but of another year: another movie.
        video(2, "Movies/Wumbo.Larc.2020.1080p.WEB-DL.mkv", 95.0),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(1001, "The Zibble Knights", Some(2025), None),
            kb_movie(1002, "Wumbo Lark", Some(2014), None),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(None, 2), (Some(1001), 1)]);
}

#[tokio::test]
async fn a_movie_by_its_name_and_release_isnt_taken_for_a_series_it_finds_no_movie_by() {
    let files = [
        // A release's main file that says its year: a movie's, though TMDB
        // finds only a series titled so, of another year.
        video(1, "Zibble.Night.2021.1080p.BluRay.x264-Blorp.mkv", 95.0),
        // A series of its year, e.g. a special TMDB lists as one.
        video(2, "Wumbo.Gala.2019.1080p.WEB-DL.x264-Blorp.mkv", 120.0),
    ];
    let kb = CatalogKb {
        series: vec![
            kb_series(1401, "Zibble Night", Some(2015)),
            kb_series(1402, "Wumbo Gala", Some(2019)),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let series: Vec<(Option<u32>, Vec<i64>)> = catalog
        .tv_series
        .iter()
        .map(|series| (series.tmdb_id, series.file_ids().map(|id| id.0).collect()))
        .collect();
    assert_eq!(series, vec![(Some(1402), vec![2])]);
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(id, files)| (id, files[0].0))
        .collect();
    assert_eq!(movies, vec![(None, 1)]);
}

#[tokio::test]
async fn a_movies_sequel_isnt_split_as_a_series_and_its_episode() {
    // Numbered like an episode before more words, but a movie's file.
    let files = [video(
        1,
        "Zibble.5.The.Final.Night.1994.1080p.BluRay.x264-Blorp.mkv",
        80.0,
    )];
    let kb = CatalogKb {
        series: vec![kb_series(1501, "Zibble", Some(1990))],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    assert!(catalog.tv_series.is_empty());
}

#[tokio::test]
async fn a_runtime_match_titled_worse_than_a_movie_the_file_may_be_a_cut_of_is_not_it() {
    let files = [
        // A TV edit of "Wumbo", as long as "Outlaw Wumbo" of its year.
        video(1, "Movies/Wumbo.1999.mkv", 96.0),
        // "The Glimmer" of its year isn't timed, so says nothing against
        // the runtime of "Fnord's The Glimmer".
        video(2, "Movies/The.Glimmer.2023.mkv", 123.0),
        // "Yonder" of the year before may be a cut of it, but "Yonder of
        // Zibble Lark" is of its year and as long.
        video(3, "Movies/Yonder.2014.mkv", 90.0),
        // "The Zorp" of its year is titled more like it, but far too short
        // to be another cut of it, as "Birdman" was of Iñárritu's.
        video(4, "Movies/Zorp.2014.mkv", 119.3),
    ];
    let kb = CatalogKb {
        movies: vec![
            kb_movie(101, "Outlaw Wumbo", Some(1999), Some(96)),
            kb_movie(102, "Wumbo", Some(1999), Some(103)),
            kb_movie(201, "Fnord's The Glimmer", Some(2023), Some(123)),
            kb_movie(202, "The Glimmer", Some(2023), None),
            kb_movie(301, "Yonder of Zibble Lark", Some(2014), Some(90)),
            kb_movie(302, "Yonder", Some(2013), Some(77)),
            kb_movie(
                401,
                "Zorp or (The Unexpected Virtue of Fnords)",
                Some(2014),
                Some(120),
            ),
            kb_movie(402, "The Zorp", Some(2014), Some(76)),
        ],
        ..CatalogKb::default()
    };
    let catalog = catalog_with_kb(&files, &[], &kb).await;
    let movies: Vec<(Option<u32>, i64)> = movies_by_id(&catalog)
        .into_iter()
        .map(|(tmdb_id, files)| (tmdb_id, files[0].0))
        .collect();
    assert_eq!(
        movies,
        vec![
            (Some(102), 1),
            (Some(201), 2),
            (Some(301), 3),
            (Some(401), 4)
        ]
    );
}
