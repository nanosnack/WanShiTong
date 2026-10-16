use super::*;

fn movie(id: u32, title: &str) -> MovieRecord {
    MovieRecord {
        id: ExternalId::Tmdb(id),
        title: title.into(),
        original_title: title.into(),
        release_year: None,
        runtime_minutes: None,
        imdb_id: None,
    }
}

fn tv(id: u32, name: &str) -> SeriesRecord {
    SeriesRecord {
        id: ExternalId::Tmdb(id),
        name: name.into(),
        original_name: name.into(),
        first_air_year: None,
        seasons: Vec::new(),
    }
}

fn title_rank(claims: &[Ranked<ClaimPredicate>]) -> f32 {
    claims
        .iter()
        .find(|c| {
            matches!(
                c.value,
                ClaimPredicate::MovieTitle(_) | ClaimPredicate::TvSeriesName(_)
            )
        })
        .unwrap()
        .rank
        .as_number()
        .unwrap()
}

#[test]
fn logs_the_first_results_of_a_search() {
    let mut results: Vec<_> = (1..=7)
        .map(|id| movie(id, &format!("Movie {id}")))
        .collect();
    results[0].release_year = Some(2018);

    let KbQueryEvent::MovieSearch { query, results } = movie_search_event("code geass", &results)
    else {
        panic!("not a movie search");
    };

    assert_eq!(query, "code geass");
    let ids: Vec<_> = results.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, (1..=5).map(ExternalId::Tmdb).collect::<Vec<_>>());
    assert_eq!(results[0].year, Some(2018));
    assert_eq!(results[1].year, None);
}

#[test]
fn either_title_counts_for_the_title_bonus() {
    let bonus = |titles| title_match_bonus("グリム童話 金の鳥", titles);
    assert_eq!(
        bonus(["The Golden Bird", "グリム童話 金の鳥"]),
        TITLE_SIMILARITY_BONUS
    );
    assert_eq!(bonus(["The Golden Bird", "The Golden Bird"]), 0.0);
}

/// The movies of the Code Geass trilogy are about as long as each other,
/// and TMDB lists Transgression first when searching for Initiation.
#[test]
fn prefers_the_best_titled_of_the_results_as_long_as_the_file() {
    let candidate = |title, runtime_minutes| Candidate {
        titles: [title, title],
        runtime_minutes: Some(runtime_minutes),
        year_agrees: false,
    };
    let results = [
        candidate(
            "Code Geass: Lelouch of the Rebellion II - Transgression",
            136,
        ),
        candidate("Code Geass: Lelouch of the Rebellion I - Initiation", 135),
        candidate(
            "Code Geass: Lelouch of the Rebellion III - Glorification",
            140,
        ),
        // Titled more like it, but not as long.
        candidate("Code Geass: Lelouch of the Rebellion", 25),
        Candidate {
            titles: ["Code Geass: Lelouch of the Rebellion - Initiation", ""],
            runtime_minutes: None,
            year_agrees: false,
        },
    ];
    let file = Duration::from_secs(2 * 3600 + 14 * 60 + 56);

    let matched = best_runtime_match(
        "code geass - lelouch of the rebellion - initiation",
        file,
        results,
        |_, _| true,
    )
    .unwrap();

    assert_eq!((matched.index, matched.minute_diff), (1, 0));
    // A few minutes off matches only if something else says it's the movie.
    let few_off = Duration::from_secs(2 * 3600 + 11 * 60);
    let matched = best_runtime_match("code geass", few_off, results, |_, _| true);
    assert_eq!(matched.map(|m| (m.index, m.minute_diff)), Some((1, 4)));
    let close_only = |_: usize, minute_diff: i64| minute_diff <= 2;
    assert_eq!(
        best_runtime_match("code geass", few_off, results, close_only),
        None
    );
    // Nothing as long as the file, nothing matched.
    let too_long = Duration::from_secs(3 * 3600);
    assert_eq!(
        best_runtime_match(
            "code geass",
            too_long,
            [candidate(
                "Code Geass: Lelouch of the Rebellion II - Transgression",
                136
            )],
            |_, _| true,
        ),
        None
    );
}

fn season(number: u32, names: &[&str]) -> Season {
    Season {
        number,
        episodes: names
            .iter()
            .enumerate()
            .map(|(i, name)| Episode {
                number: i as u32 + 1,
                name: (*name).into(),
                runtime_minutes: None,
                production_code: None,
                aired: None,
            })
            .collect(),
    }
}

#[test]
fn an_episode_numbered_through_the_series_is_in_the_season_that_counts_to_it() {
    let summary = |number: u32, episode_count: u32| SeasonSummary {
        number,
        name: format!("Season {number}"),
        episode_count,
    };
    // One Piece's first seasons, after its specials, out of order.
    let seasons = [
        summary(2, 16),
        summary(0, 39),
        summary(1, 61),
        summary(3, 14),
    ];
    let season_of = |first| season_numbered_through(&seasons, first);

    assert_eq!(season_of(1), Some((1, 0)));
    assert_eq!(season_of(61), Some((1, 60)));
    assert_eq!(season_of(62), Some((2, 0)));
    assert_eq!(season_of(73), Some((2, 11)));
    assert_eq!(season_of(91), Some((3, 13)));
    // Past the last season.
    assert_eq!(season_of(92), None);
}

#[test]
fn an_episode_is_the_one_named_like_the_files_title() {
    let seasons = [
        season(0, &["Jack Webb on the Farewell Special (1974)"]),
        season(1, &["Pilot", "Homicide: Who Killed Who", "The Bubble"]),
        season(2, &["The Bubble", "Alibi Me"]),
    ];
    let titled = |title| episode_titled(&seasons, title).map(|e| (e.season, e.episode, e.name));
    // A name's year in parentheses, which a file's title is without.
    assert_eq!(
        titled("Jack Webb on the Farewell Special"),
        Some((0, 1, "Jack Webb on the Farewell Special (1974)".into()))
    );
    assert_eq!(
        titled("Homicide-Who Killed Who"),
        Some((1, 2, "Homicide: Who Killed Who".into()))
    );
    assert_eq!(titled("alibi me"), Some((2, 2, "Alibi Me".into())));
    // Two episodes have the name, so neither is the file.
    assert_eq!(titled("The Bubble"), None);
    // Some of a name's words aren't it.
    assert_eq!(titled("Who Killed Who"), None);
}

#[test]
fn a_title_renumbers_a_file_only_when_its_numbers_say_another_episode() {
    let episode = || TitledEpisode {
        season: 2,
        episode: 7,
        name: "Alibi Me".into(),
    };
    let numbered = |claims: Vec<Ranked<ClaimPredicate>>| -> Vec<ClaimPredicate> {
        claims
            .into_iter()
            .map(|claim| claim.value)
            .filter(|claim| {
                matches!(
                    claim,
                    ClaimPredicate::Season(_)
                        | ClaimPredicate::EpisodeRange(_)
                        | ClaimPredicate::EpisodeTitle(_)
                )
            })
            .collect()
    };
    let rank = Rank::Number(59.0);
    assert_eq!(
        numbered(titled_episode_claims(
            rank,
            episode(),
            Some(2),
            Some([7, 7])
        )),
        vec![ClaimPredicate::EpisodeTitle("Alibi Me".into())],
    );
    assert_eq!(
        numbered(titled_episode_claims(
            rank,
            episode(),
            Some(1),
            Some([30, 30])
        )),
        vec![
            ClaimPredicate::EpisodeTitle("Alibi Me".into()),
            ClaimPredicate::Season(2),
            ClaimPredicate::EpisodeRange([7, 7]),
        ],
    );
}

/// TMDB has an entry of its own for Kingdom of Heaven's director's cut,
/// beside the movie's, and only it is as long as a file of that cut.
#[test]
fn an_entry_for_a_cut_of_another_movie_is_that_movie_in_an_edition() {
    let details = |id: u32, title: &str, year: u32, runtime: u32| MovieRecord {
        release_year: Some(year),
        runtime_minutes: Some(runtime),
        ..movie(id, title)
    };
    let results = [
        details(1495, "Kingdom of Heaven", 2005, 144),
        details(1759035, "Kingdom Of Heaven (Director's Cut)", 2005, 194),
        details(9999, "Kingdom of Heaven", 1989, 90),
    ];

    assert_eq!(
        cut_of_another(&results, 1),
        Some((0, "Director's Cut".into())),
        "the movie released closest to the cut",
    );
    // The movie isn't a cut of anything.
    assert_eq!(cut_of_another(&results, 0), None);
    // Without the movie among the results, the cut's entry is all there is.
    assert_eq!(cut_of_another(&results[1..2], 0), None);
}

#[test]
fn a_file_can_be_another_cut_but_not_an_episode_of_a_movie() {
    let fit = |file_minutes: u64, tmdb_minutes| {
        runtime_fit(Duration::from_secs(file_minutes * 60), tmdb_minutes)
    };
    assert_eq!(fit(170, Some(170)), RuntimeFit::Match { minute_diff: 0 });
    // PlayTime's 124-minute cut, and Das Boot's director's cut.
    assert_eq!(fit(124, Some(115)), RuntimeFit::OtherCut);
    assert_eq!(fit(209, Some(149)), RuntimeFit::OtherCut);
    // A Chibi Maruko-chan episode named `11.MP4`, for "11+".
    assert_eq!(fit(22, Some(90)), RuntimeFit::Contradicts);
    assert_eq!(fit(400, Some(149)), RuntimeFit::Contradicts);
    assert_eq!(fit(22, None), RuntimeFit::Unknown);
}

#[test]
fn without_a_runtime_match_falls_back_to_what_the_file_could_be() {
    let candidate = |title, runtime_minutes| Candidate {
        titles: [title, title],
        runtime_minutes,
        year_agrees: false,
    };
    let episode = Duration::from_secs(21 * 60 + 11);
    // What TMDB finds for an episode's number, all features.
    let features = [
        candidate("11+", Some(90)),
        candidate("Apollo 11", Some(93)),
        candidate("11:14", Some(86)),
    ];
    assert_eq!(best_unmatched_candidate("11", episode, features), None);

    // Titled less like it, but it might be it.
    let with_unknown = [candidate("11+", Some(90)), candidate("Eleven", None)];
    assert_eq!(
        best_unmatched_candidate("11", episode, with_unknown).map(|u| (u.index, u.fit)),
        Some((1, RuntimeFit::Unknown))
    );

    let playtime = [
        candidate("Playtime Towers", Some(118)),
        candidate("PlayTime", Some(115)),
    ];
    assert_eq!(
        best_unmatched_candidate("playtime", Duration::from_secs(124 * 60), playtime)
            .map(|u| (u.index, u.fit)),
        Some((1, RuntimeFit::OtherCut))
    );
}

#[test]
fn a_runtime_match_is_corroborated_by_the_year_or_a_title() {
    let corroborates = |query, file_year, title, movie_year| {
        corroborates_runtime_match(query, file_year, [title, title], movie_year)
    };
    // The years agree, e.g. `Heat.1995.mkv`, or are one apart, e.g.
    // `300.2006` for TMDB's 2007 release.
    assert!(corroborates("heat", Some(1995), "Heat", Some(1995)));
    assert!(corroborates("300", Some(2006), "300", Some(2007)));
    assert!(!corroborates("300", Some(2004), "300", Some(2007)));
    // A title of the movie's, without a year.
    assert!(corroborates(
        "code geass - lelouch of the rebellion - initiation",
        None,
        "Code Geass: Lelouch of the Rebellion I - Initiation",
        Some(2017),
    ));
    // A number's no title, as Chibi Maruko-chan's `01.MP4` is none of
    // `Ash 01` or `01`.
    assert!(!corroborates("01", None, "Ash 01", Some(2016)));
    assert!(!corroborates("29", None, "29", Some(2025)));
    assert!(!corroborates("46", None, "Swimsuit 46", Some(2011)));
}

#[test]
fn finds_the_season_named_like_a_name_the_file_gives() {
    let season = |number, name: &str, episode_count| SeasonSummary {
        number,
        name: name.into(),
        episode_count,
    };
    // Some of One Pace's seasons, as TMDB lists them.
    let one_pace = [
        season(1, "Romance Dawn", 4),
        season(14, "Alabasta", 21),
        season(33, "Whole Cake Island", 0),
        season(34, "Reverie", 0),
        season(35, "Wano", 0),
    ];

    assert_eq!(
        season_named_like(&one_pace, &["whole cake island"], [1, 1]),
        Some(33)
    );
    // No episodes listed, so any number will do.
    assert_eq!(
        season_named_like(&one_pace, &["one pace", "wano"], [52, 52]),
        Some(35)
    );
    // More episodes than the season has.
    assert_eq!(
        season_named_like(&one_pace, &["romance dawn"], [5, 5]),
        None
    );
    assert_eq!(
        season_named_like(&one_pace, &["romance dawn"], [4, 4]),
        Some(1)
    );
    // Spelled otherwise.
    assert_eq!(season_named_like(&one_pace, &["arabasta"], [1, 1]), None);
}

#[test]
fn prefers_the_series_titled_as_searched_for_over_one_its_part_of() {
    let chosen = |query, results: &[SeriesRecord]| {
        series_titled_claims(query, Rank::Number(50.0), results)
            .unwrap()
            .into_iter()
            .find_map(|claim| match claim.value {
                ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(id)) => Some(id),
                _ => None,
            })
    };
    // As TMDB lists them.
    let after_life = [
        tv(197406, "Life After Life"),
        tv(79410, "After Life"),
        tv(79872, "The Class: Life After"),
    ];
    assert_eq!(chosen("after life", &after_life), Some(79410));
    // A subtitle, e.g. the anime's, is more than the name, not other than it.
    let food_wars = [
        tv(62273, "Food Wars! Shokugeki no Soma"),
        tv(323270, "Food Wars"),
        tv(32354, "Food Wars"),
    ];
    assert_eq!(chosen("food wars", &food_wars), Some(62273));
    // None titled so, e.g. a name that's part of theirs.
    let office = [tv(2316, "The Office"), tv(2996, "The Office UK")];
    assert_eq!(chosen("office", &office), Some(2316));
}

/// A 124-minute cut of PlayTime, which TMDB lists at 115 minutes, so no
/// search result matches on runtime: the movie titled so over a series
/// titled that and more.
#[test]
fn prefers_the_search_result_titled_as_searched_for() {
    let movie_claims = chosen_movie_claims(
        "playtime",
        None,
        Rank::Number(50.0),
        &[movie(10227, "PlayTime"), movie(846813, "Playtime")],
    )
    .unwrap();
    let tv_claims = series_titled_claims(
        "playtime",
        Rank::Number(50.0),
        &[tv(274519, "Playtime Towers")],
    )
    .unwrap();

    assert!(title_rank(&movie_claims) > title_rank(&tv_claims));
}

/// Specials as TMDB lists them: number, name and minutes.
fn specials(listed: &[(u32, &str, u32)]) -> Vec<Episode> {
    listed
        .iter()
        .map(|&(number, name, runtime)| Episode {
            number,
            name: name.into(),
            runtime_minutes: Some(runtime),
            production_code: None,
            aired: None,
        })
        .collect()
}

fn minutes(minutes: u64, seconds: u64) -> Duration {
    Duration::from_secs(minutes * 60 + seconds)
}

#[test]
fn a_special_is_the_one_whose_name_the_files_title_has() {
    let taskmaster = specials(&[
        (1, "Taskmaster's New Year Treat 2021", 45),
        (2, "Taskmaster's New Year Treat 2022", 45),
        (3, "Taskmaster's New Year Treat 2023", 48),
        (4, "Taskmaster's New Year Treat 2024", 48),
    ]);
    let new_year = |year, runtime| {
        matching_special(
            &taskmaster,
            &["taskmasters new year treat"],
            Some(year),
            runtime,
        )
    };
    assert_eq!(new_year(2022, minutes(47, 13)), Some(2));
    assert_eq!(new_year(2024, minutes(47, 56)), Some(4));

    // Its name before a subtitle will do.
    let untold = specials(&[
        (
            1,
            "Prologue: Chapter A - World War I, the Russian Revolution & Woodrow Wilson",
            58,
        ),
        (
            2,
            "Prologue: Chapter B - 1920-1940, Franklin D. Roosevelt, Adolf Hitler and Joseph Stalin",
            58,
        ),
        (3, "A Conversation with History", 58),
        (4, "Episode 4", 25),
    ]);
    let title = "oliver stones untold history of the united states s01 bonus prologue chapter b 720p bluray x264-wibble";
    assert_eq!(
        matching_special(&untold, &[title], None, minutes(56, 14)),
        Some(2)
    );
}

#[test]
fn a_special_named_in_a_word_is_one_only_as_long_as_the_file() {
    let wire = specials(&[
        (1, "It's All Connected", 29),
        (2, "When Bunk Met McNulty (2000)", 3),
        (5, "Odyssey", 29),
        (6, "The Last Word", 27),
        (7, "Tapping The Wire", 25),
    ]);
    let special = |titles: &[&str], runtime| matching_special(&wire, titles, None, runtime);
    // Any of the file's titles, e.g. its name's rather than its tag's.
    assert_eq!(
        special(
            &["its all connected", "the wire season 4 disc 3"],
            minutes(28, 50)
        ),
        Some(1)
    );
    assert_eq!(special(&["the wire odyssey"], minutes(28, 42)), Some(5));
    assert_eq!(special(&["the last word"], minutes(26, 36)), Some(6));
    // Not a reunion as long as a movie, nor featurettes TMDB doesn't list.
    assert_eq!(special(&["the wire odyssey"], minutes(85, 27)), None);
    assert_eq!(special(&["the game is real"], minutes(30, 2)), None);
}

#[test]
fn specials_named_alike_are_neither_the_file() {
    let listed = specials(&[(1, "Recap", 24), (2, "Recap", 24), (3, "Recap Special", 24)]);
    assert_eq!(
        matching_special(&listed, &["show recap"], None, minutes(24, 0)),
        None
    );
    // Unless one's name has more of the title's words.
    assert_eq!(
        matching_special(&listed, &["show recap special"], None, minutes(24, 0)),
        Some(3)
    );
}

/// Search results as TMDB gives them, when no runtime says which work the
/// file is: one titled as searched for and of the file's year over the
/// knowledge base's first, for a movie and a series alike.
#[test]
fn without_a_runtime_a_work_titled_so_and_of_the_files_year_is_chosen() {
    let dated = |id, title: &str, year| MovieRecord {
        release_year: Some(year),
        ..movie(id, title)
    };
    let chosen = |query, year, results: &[MovieRecord]| {
        choose_movie_result(query, year, results).map(|movie| movie.id.clone())
    };
    // A search for 2018 finds the original first, for its re-release that year.
    let halloween = [
        dated(948, "Halloween", 1978),
        dated(424139, "Halloween", 2018),
    ];
    assert_eq!(
        chosen("halloween", Some(2018), &halloween),
        Some(ExternalId::Tmdb(424139))
    );
    // A year a festival's or a release's off.
    assert_eq!(
        chosen("halloween", Some(2019), &halloween),
        Some(ExternalId::Tmdb(424139))
    );
    // The title over a longer one with it in.
    let friday = [
        dated(9725, "Friday the 13th Part 2", 1981),
        dated(4488, "Friday the 13th", 1980),
    ];
    assert_eq!(
        chosen("friday the 13th", Some(1980), &friday),
        Some(ExternalId::Tmdb(4488))
    );
    // Without the file's year, the title alone says too little: the first.
    assert_eq!(
        chosen("friday the 13th", None, &friday),
        Some(ExternalId::Tmdb(9725))
    );

    // A series alike: a spin-off of the year is no match for its name.
    let dated_series = |id, name: &str, year| SeriesRecord {
        first_air_year: Some(year),
        ..tv(id, name)
    };
    let drag = [
        dated_series(38409, "Drag Race: Untucked", 2010),
        dated_series(8514, "Drag Race", 2009),
    ];
    let chosen_series = |year| choose_series_result("drag race", year, &drag).map(|s| s.id.clone());
    // Listed first, and of the very year, but not what was searched for.
    assert_eq!(chosen_series(Some(2010)), Some(ExternalId::Tmdb(8514)));
    // Without a year, the one named as searched for over one named that and
    // more, listed first.
    let zibble = [tv(21, "Zibble Together"), tv(22, "Zibble")];
    let chosen = choose_series_result("zibble", None, &zibble).map(|s| s.id.clone());
    assert_eq!(chosen, Some(ExternalId::Tmdb(22)));
}

#[test]
fn a_title_is_a_series_name_and_an_episode_number_at_its_end() {
    // The likeliest split's name, season and episode.
    let split = |title| {
        series_and_episode(title)
            .into_iter()
            .next()
            .map(|split| (split.name, split.season, split.episode))
    };
    let named = |name: &str, episode| Some((name.to_string(), None, episode));
    assert_eq!(
        split("samurai sentai blorp 22 hd"),
        named("samurai sentai blorp", 22)
    );
    assert_eq!(split("wumbo lark 06sd"), named("wumbo lark", 6));
    assert_eq!(split("zibble 927 sub ita"), named("zibble", 927));
    assert_eq!(
        split("glimmer peaks e01 hd1080 bluray"),
        named("glimmer peaks", 1)
    );
    assert_eq!(
        split("glimmer peaks episode 2 vostfr"),
        named("glimmer peaks", 2)
    );
    assert_eq!(split("yonder - 13"), named("yonder", 13));
    // Followed by the episode's own title.
    assert_eq!(split("glimmer cx 260 majuuou"), named("glimmer cx", 260));
    assert_eq!(
        split("wumbo lark 16 - the bright road (dvd x264 aac)"),
        named("wumbo lark", 16)
    );
    assert_eq!(
        split("zibble road folge 7 (ein stuck)"),
        named("zibble road", 7)
    );
    // A season written before it.
    assert_eq!(
        split("zibble deka s1 _ 13"),
        Some(("zibble deka".to_string(), Some(1), 13))
    );
    // Another's number: a volume's, a day's, a part's, a movie's.
    assert_eq!(split("zibble vol 1"), None);
    assert_eq!(split("wumbo live day 2"), None);
    assert_eq!(split("glimmer the movie 3"), None);
    assert_eq!(split("zibble road part 1"), None);
    // A year, a number alone, and no number.
    assert_eq!(split("zibble 2019"), None);
    assert_eq!(split("22 hd"), None);
    assert_eq!(split("zibble road"), None);
    // A number at the end first, then the first with words before.
    let names = |title| -> Vec<String> {
        series_and_episode(title)
            .into_iter()
            .map(|split| split.name)
            .collect()
    };
    assert_eq!(
        names("wumbo 0083 glimmer road 01"),
        vec!["wumbo 0083 glimmer road", "wumbo"]
    );
}

#[test]
fn a_title_is_a_series_name_and_a_season_at_its_end() {
    let split = |title| series_and_season(title);
    let named = |name: &str, season| Some((name.to_string(), season));
    assert_eq!(split("wumbo lark 2nd season"), named("wumbo lark", 2));
    assert_eq!(split("zibble s02"), named("zibble", 2));
    assert_eq!(split("zibble season 3 1080p"), named("zibble", 3));
    assert_eq!(split("glimmer peaks iv"), named("glimmer peaks", 4));
    assert_eq!(
        split("glimmer peaks fourth stage"),
        named("glimmer peaks", 4)
    );
    assert_eq!(split("yonder - s2"), named("yonder", 2));
    // No season, or nothing before it.
    assert_eq!(split("zibble road"), None);
    assert_eq!(split("s02"), None);
    assert_eq!(split("zibble part 2"), None);
}

/// Of two movies titled alike and about as long as the file, the one of its
/// year, though the other is a minute closer.
#[test]
fn prefers_the_file_s_year_to_a_closer_runtime() {
    let candidate = |runtime_minutes, year_agrees| Candidate {
        titles: ["Wumbo Lark", "Wumbo Lark"],
        runtime_minutes: Some(runtime_minutes),
        year_agrees,
    };
    let file = Duration::from_secs(102 * 60);
    let results = [candidate(101, false), candidate(99, true)];

    let matched = best_runtime_match("wumbo lark", file, results, |_, _| true);

    assert_eq!(matched.map(|m| m.index), Some(1));
}

/// A run named as a season of its own is the season named as its own words,
/// or else the one left as long as it, or else the one left. A run of its
/// series' own is numbered through it.
#[test]
fn a_run_named_as_a_season_of_its_own_is_the_season_named_so_or_as_long_as_it() {
    use RunNumbering::{Season, Through};
    let seasons = |names_and_counts: &[(&str, u32)]| -> Vec<SeasonSummary> {
        std::iter::once(("Specials", 5))
            .chain(names_and_counts.iter().copied())
            .enumerate()
            .map(|(number, (name, episode_count))| SeasonSummary {
                number: number as u32,
                name: name.to_string(),
                episode_count,
            })
            .collect()
    };
    let run = |name: &str, episodes, family: &[(&str, u32)]| EpisodeRun {
        name: name.to_string(),
        episodes,
        last: episodes,
        family: family
            .iter()
            .map(|&(name, n)| (name.to_string(), n))
            .collect(),
    };
    let named = |name: &str, seasons: &[SeasonSummary], run: &EpisodeRun, found_by| {
        let series = SeriesRecord {
            id: ExternalId::Tmdb(1),
            name: name.to_string(),
            original_name: "ウンボ・ラーク".to_string(),
            first_air_year: Some(2001),
            seasons: seasons.to_vec(),
        };
        run_numbering(&series, run, found_by, [1, 1])
    };
    let numbering = |seasons: &[SeasonSummary], run: &EpisodeRun, found_by| {
        named("Wumbo Lark", seasons, run, found_by)
    };
    // Three seasons as long: one named as a run's own words, the other left.
    let as_long = seasons(&[("Season 1", 12), ("Season 2", 12), ("Bloom", 12)]);
    let family = [
        ("wumbo lark", 12),
        ("wumbo lark ~bloom~", 12),
        ("wumbo lark??", 12),
    ];
    let bloom = run("wumbo lark ~bloom~", 12, &family);
    let again = run("wumbo lark??", 12, &family);
    assert_eq!(numbering(&as_long, &bloom, None), Season(Some(3)));
    assert_eq!(numbering(&as_long, &again, None), Season(Some(2)));
    // The first keeps its numbers.
    assert_eq!(
        numbering(&as_long, &run("wumbo lark", 12, &family), None),
        Through
    );
    // Alone in its release, named as its series and a season, however it
    // was found: that season. Named as its series and more that no season
    // is: through the series, unless found by a shorter name, when only
    // a release's words say nothing.
    let parts = seasons(&[("Glimmer", 26), ("Fnord Nights", 39), ("Yonder", 39)]);
    let fnord = run("wumbo lark - fnord nights", 39, &[]);
    assert_eq!(
        numbering(&parts, &fnord, Some("wumbo lark - fnord nights")),
        Season(Some(2))
    );
    assert_eq!(numbering(&parts, &fnord, None), Season(Some(2)));
    let other = run("wumbo lark tv-2", 39, &[]);
    assert_eq!(numbering(&parts, &other, None), Through);
    assert_eq!(numbering(&parts, &other, Some("wumbo lark")), Season(None));
    let complete = run("wumbo lark complete 720p", 104, &[]);
    assert_eq!(numbering(&parts, &complete, Some("wumbo lark")), Through);
    // Named so at its end, or the one left beside its first.
    let clannad = seasons(&[("Wumbo Lark", 22), ("Wumbo Lark: Yonder Story", 22)]);
    let family = [("wumbo lark", 24), ("wumbo lark ~yonder story~", 25)];
    let yonder = run("wumbo lark ~yonder story~", 25, &family);
    assert_eq!(numbering(&clannad, &yonder, None), Season(Some(2)));
    let other = seasons(&[("Season 1", 12), ("Heart Throb", 12)]);
    let family = [("wumbo lark", 13), ("wumbo lark ren", 13)];
    let ren = run("wumbo lark ren", 13, &family);
    assert_eq!(numbering(&other, &ren, None), Season(Some(2)));
    // A first longer than the whole series isn't its first: the nearer one
    // is, or else the run is the series' own.
    let crowds = seasons(&[("Season 1", 12), ("Wumbo Lark Yonder Glimmer", 13)]);
    let family = [
        ("wumbo lark", 105),
        ("wumbo lark yonder", 12),
        ("wumbo lark yonder glimmer", 13),
    ];
    let yonder = run("wumbo lark yonder", 12, &family);
    let glimmer = run("wumbo lark yonder glimmer", 13, &family);
    let yonder_named = |run: &EpisodeRun| named("Wumbo Lark Yonder", &crowds, run, None);
    assert_eq!(yonder_named(&yonder), Through);
    assert_eq!(yonder_named(&glimmer), Season(Some(2)));
    // Neither named so, nor as long as one left, nor the one run left for
    // one season: unknown.
    let named = seasons(&[("Wumbo Lark", 46), ("X", 38), ("Y", 39), ("W", 30)]);
    let family = [
        ("wumbo lark", 46),
        ("wumbo lark z", 40),
        ("wumbo lark x", 38),
    ];
    assert_eq!(
        numbering(&named, &run("wumbo lark z", 40, &family), None),
        Season(None)
    );
}
