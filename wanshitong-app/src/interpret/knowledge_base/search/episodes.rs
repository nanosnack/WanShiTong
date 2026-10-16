//! Finding a series' episode or special that a file is, by its numbers, its title or its runtime.

use super::*;
use crate::interpret::release_words;
use std::collections::HashMap;

/// The number of the series' special that the file is, as
/// [`matching_special`] finds it among the series' specials, its season 0.
pub(super) async fn special_named_like(
    kb: &dyn KnowledgeBaseApi,
    series: &ExternalId,
    fields: &RankedSearchFields,
    kb_queries: &mut Vec<KbQueryEvent>,
) -> Result<Option<u32>> {
    let titles: Vec<&str> = fields.titles.iter().map(|t| t.value.as_str()).collect();
    let Some(runtime) = fields.runtime.filter(|_| !titles.is_empty()) else {
        return Ok(None);
    };
    let Some(specials) = kb.season(series, 0, kb_queries).await? else {
        return Ok(None);
    };
    Ok(matching_special(
        &specials.episodes,
        &titles,
        fields.release_year,
        runtime,
    ))
}

/// The episode of the series that a file titled `title` is, as
/// [`episode_titled`] finds it: in the season the file's numbers say, if
/// any, then in all the series' seasons.
pub(super) async fn episode_titled_like(
    kb: &dyn KnowledgeBaseApi,
    series: &ExternalId,
    title: &str,
    season: Option<u32>,
    kb_queries: &mut Vec<KbQueryEvent>,
) -> Result<Option<TitledEpisode>> {
    let mut seasons = Vec::new();
    if let Some(season) = season {
        if let Some(details) = kb.season(series, season, kb_queries).await? {
            seasons.push(details);
        }
        if let Some(episode) = episode_titled(&seasons, title) {
            return Ok(Some(episode));
        }
    }
    let Some(record) = kb.series(series, kb_queries).await? else {
        return Ok(None);
    };
    for summary in &record.seasons {
        if Some(summary.number) == season {
            continue;
        }
        if let Some(details) = kb.season(series, summary.number, kb_queries).await? {
            seasons.push(details);
        }
    }
    Ok(episode_titled(&seasons, title))
}

/// The season and episodes on the knowledge base of a file numbered through
/// the whole series, e.g. One Piece's `E73`, in the season
/// [`season_numbered_through`] finds its first in: its first's number as the
/// knowledge base gives it, and the rest counted on from it, e.g. 77 and 78
/// for `E77-E78`.
pub(super) async fn episodes_numbered_through(
    kb: &dyn KnowledgeBaseApi,
    series: &ExternalId,
    episodes: [u32; 2],
    kb_queries: &mut Vec<KbQueryEvent>,
) -> Result<Option<(u32, [u32; 2])>> {
    let Some(record) = kb.series(series, kb_queries).await? else {
        return Ok(None);
    };
    let [first, last] = episodes;
    let Some((season, first_in)) = season_numbered_through(&record.seasons, first) else {
        return Ok(None);
    };
    let Some(details) = kb.season(series, season, kb_queries).await? else {
        return Ok(None);
    };
    Ok(details
        .episodes
        .get(first_in)
        .map(|episode| (season, [episode.number, episode.number + (last - first)])))
}

/// The season that the first episode of a file numbered through the whole
/// series is in, counting the episodes of its seasons in order, without its
/// specials, and its position in it, e.g. season 2's 12th for One Piece's
/// `E73-E74`, as its season 1 has 61. A file of episodes that two seasons
/// share is in its first's, e.g. season 2 for `E77-E78`, as season 2 ends at
/// 77: it's one file, so it's numbered as one. Not past the last season.
pub(super) fn season_numbered_through(
    seasons: &[SeasonSummary],
    first: u32,
) -> Option<(u32, usize)> {
    let mut seasons: Vec<&SeasonSummary> =
        seasons.iter().filter(|season| season.number > 0).collect();
    seasons.sort_by_key(|season| season.number);
    let mut before = 0;
    for season in seasons {
        let through = before + season.episode_count;
        if first <= through {
            let first_in = first.checked_sub(before + 1)?;
            return Some((season.number, first_in as usize));
        }
        before = through;
    }
    None
}

/// An episode that a file's title names.
#[derive(Debug, PartialEq)]
pub(super) struct TitledEpisode {
    pub(super) season: u32,
    pub(super) episode: u32,
    /// Its name, as the knowledge base writes it.
    pub(super) name: String,
}

/// The episode, of the seasons', that a file's title names, if it names
/// exactly one:
///
/// 1. the one whose name has the words of the title, in order, e.g.
///    `Homicide: Who Killed Who` for `Homicide-Who Killed Who`. A name's
///    words in parentheses may be left out, e.g. `(1974)` in `… Second
///    Farewell Special (1974)`, as a file's title is without them;
/// 2. else the one whose production code the title has, e.g. `DR-19` in
///    `Juvenile Division-DR-19`, as some releases name their files by it. A
///    code of digits alone, e.g. `101`, is too like a number in a title to
///    go by;
/// 3. else the one whose name's words the title has, in their order, with
///    others between, e.g. `Juvenile: DR-19` in `Juvenile Division-DR-19`.
///    Not a name of one word, which too many titles have;
/// 4. else the one whose name begins as the title does and has the rest of
///    its words in order, with others between, e.g. `Mermaid Man and
///    Barnacle Boy V` for `Mermaid and Barnacle Boy V`, where the release
///    dropped a word. Not a title of one word, likewise, nor one that's only
///    the end of a name, e.g. `Who Killed Who` for `Homicide: Who Killed
///    Who`, as a subtitle is too often shared.
///
/// Each step is tried in all the seasons before the next, so a sure match
/// in one isn't outdone by a loose one in another.
pub(super) fn episode_titled(seasons: &[Season], title: &str) -> Option<TitledEpisode> {
    let parenthesized = Regex::new(r"\s*\([^)]*\)").unwrap();
    let named_so = |episode: &Episode| {
        title::same_words(&episode.name, title)
            || title::same_words(&parenthesized.replace_all(&episode.name, ""), title)
    };
    let coded_so = |episode: &Episode| {
        episode.production_code.as_deref().is_some_and(|code| {
            code.chars().any(char::is_alphabetic) && title::has_words_in_a_row(title, code)
        })
    };
    let named_within = |episode: &Episode| {
        title::has_words_in_order(title, &parenthesized.replace_all(&episode.name, ""))
    };
    let titled_within =
        |episode: &Episode| title::has_words_in_order_from_the_start(&episode.name, title);
    let only = |matches: &dyn Fn(&Episode) -> bool| {
        let mut found = seasons.iter().flat_map(|season| {
            season
                .episodes
                .iter()
                .filter(|episode| matches(episode))
                .map(|episode| TitledEpisode {
                    season: season.number,
                    episode: episode.number,
                    name: episode.name.clone(),
                })
        });
        let episode = found.next()?;
        found.next().is_none().then_some(episode)
    };
    only(&named_so)
        .or_else(|| only(&coded_so))
        .or_else(|| only(&named_within))
        .or_else(|| only(&titled_within))
}

/// Claims of which of a series' episodes a file is, beyond its own
/// numbers: the episode its title names, as [`episode_titled_like`] finds
/// it, if it's of one episode, or else, numbered through the whole series,
/// the season and episodes the knowledge base has them as, as
/// [`episodes_numbered_through`] finds them, if the file's own season and
/// episodes say otherwise, e.g. a release that splits its seasons where the
/// knowledge base doesn't. However the series was found: by its name, or by
/// its ID. But a run of a release named as a season of its own, numbered
/// from 1, is that season's: see [`run_season`].
pub(super) async fn episode_claims(
    kb: &dyn KnowledgeBaseApi,
    series: &ExternalId,
    fields: &RankedSearchFields,
    found_by: Option<&str>,
    rank: Rank,
    kb_queries: &mut Vec<KbQueryEvent>,
) -> Result<Vec<Ranked<ClaimPredicate>>> {
    let &RankedSearchFields {
        season,
        episode_range,
        absolute_episode_range,
        ref episode_title,
        ..
    } = fields;
    let one_episode = episode_range.is_none_or(|[first, last]| first == last);
    if let Some(title) = episode_title
        && one_episode
        && let Some(episode) =
            episode_titled_like(kb, series, &title.value, season, kb_queries).await?
    {
        return Ok(titled_episode_claims(rank, episode, season, episode_range));
    }
    if let Some(episodes) = absolute_episode_range
        && let Some(run) = &fields.run
        && let Some(record) = kb.series(series, kb_queries).await?
        && let RunNumbering::Season(season) = run_numbering(&record, run, found_by, episodes)
    {
        return Ok(season
            .map(|season| {
                vec![
                    Ranked::new(rank + KB_BONUS, ClaimPredicate::Season(season)),
                    Ranked::new(rank + KB_BONUS, ClaimPredicate::EpisodeRange(episodes)),
                ]
            })
            .unwrap_or_default());
    }
    if let Some(episodes) = absolute_episode_range
        && let Some(run) = &fields.run
        && let Some(special) = past_first_season(kb, series, run, episodes, kb_queries).await?
    {
        return Ok(special
            .map(|special| {
                vec![
                    Ranked::new(rank + KB_BONUS, ClaimPredicate::Season(0)),
                    Ranked::new(rank + KB_BONUS, ClaimPredicate::EpisodeRange(special)),
                ]
            })
            .unwrap_or_default());
    }
    if let Some(episodes) = absolute_episode_range
        && let Some((kb_season, kb_episodes)) =
            episodes_numbered_through(kb, series, episodes, kb_queries).await?
        && (season, episode_range) != (Some(kb_season), Some(kb_episodes))
    {
        return Ok(vec![
            Ranked::new(rank + KB_BONUS, ClaimPredicate::Season(kb_season)),
            Ranked::new(rank + KB_BONUS, ClaimPredicate::EpisodeRange(kb_episodes)),
        ]);
    }
    Ok(Vec::new())
}

/// Claims for a file titled like one of its series' episodes: the episode's
/// name, and if the file's numbers say another episode, the season and
/// episode it is, outranking them, e.g. a series numbered on its DVDs in
/// another order than it aired in.
pub(super) fn titled_episode_claims(
    rank: Rank,
    episode: TitledEpisode,
    season: Option<u32>,
    episode_range: Option<[u32; 2]>,
) -> Vec<Ranked<ClaimPredicate>> {
    let rank = rank + EPISODE_TITLE_BONUS;
    let TitledEpisode {
        season: titled_season,
        episode: number,
        name,
    } = episode;
    let mut claims = vec![Ranked::new(
        rank + KB_BONUS,
        ClaimPredicate::EpisodeTitle(name),
    )];
    if (season, episode_range) != (Some(titled_season), Some([number, number])) {
        claims.extend([
            Ranked::new(rank + KB_BONUS, ClaimPredicate::Season(titled_season)),
            Ranked::new(
                rank + KB_BONUS,
                ClaimPredicate::EpisodeRange([number, number]),
            ),
        ]);
    }
    claims
}

/// The special, of `specials`, that a file is: the one whose name one of the
/// file's titles, with its year, has all the words of, e.g. `Taskmaster's New
/// Year Treat 2022` for `taskmasters new year treat` of 2022, or of its name
/// before a subtitle, e.g. `Prologue: Chapter A` of `Prologue: Chapter A -
/// World War I, …`. Only if no other special's name has as many words, and
/// it's about as long as the file, as a name can be one word, e.g. `Odyssey`.
pub(super) fn matching_special(
    specials: &[Episode],
    titles: &[&str],
    year: Option<u32>,
    runtime: Duration,
) -> Option<u32> {
    let titles: Vec<String> = titles
        .iter()
        .map(|title| match year {
            Some(year) => format!("{title} {year}"),
            None => title.to_string(),
        })
        .collect();
    // The specials named in a title, by how many words of its name it has.
    let mut named: Vec<(usize, u32)> = specials
        .iter()
        .filter(|special| {
            special
                .runtime_minutes
                .is_some_and(|minutes| runtime_is_close(runtime, minutes))
        })
        .filter_map(|special| {
            let whole = special.name.as_str();
            let before_subtitle = whole.split(" - ").next().unwrap_or(whole);
            [whole, before_subtitle]
                .into_iter()
                .filter(|name| title::has_words(name))
                .filter(|name| titles.iter().any(|t| title::coverage(t, name) == 1.0))
                .map(|name| title::words(name).len())
                .max()
                .map(|words| (words, special.number))
        })
        .collect();
    named.sort_by(|a, b| b.cmp(a));
    match named.as_slice() {
        [(most, number), rest @ ..] if rest.first().is_none_or(|(next, _)| next < most) => {
            Some(*number)
        }
        _ => None,
    }
}

/// How a run of a release's episodes numbered from 1 is numbered on the
/// knowledge base.
#[derive(Debug, PartialEq)]
pub(super) enum RunNumbering {
    /// Through the whole series, as the series' own run, e.g. `sailor moon
    /// crystal` 01 to 36 through Sailor Moon Crystal's three seasons.
    Through,
    /// As a season of its own, if it can be told which.
    Season(Option<u32>),
}

/// How a run of a release's episodes is numbered on the found `series`. As
/// a season of its own if it's named as a run of its release is and more,
/// e.g. `mahoromatic ~motto utsukushii mono~` beside `mahoromatic`, and that
/// run could be the series': not longer than the whole series, e.g. not
/// 1972's Gatchaman of 105 episodes for Gatchaman Crowds' `insight`, whose
/// first is `gatchaman crowds`. Or if it's named as the series is, or the
/// name that found it, and more, and a season is named so, e.g. JoJo's
/// Bizarre Adventure's "Golden Wind" for `jojos bizarre adventure - golden
/// wind`. Or if the series was found by a name the run's only begins with,
/// unless what its name has more is only a release's words, e.g. `detective
/// conan remastered`. Else through the series.
///
/// The season is one the release's other such runs of the series aren't,
/// nor the first, if the run it's named after is in the release:
///
/// 1. the season named as the words its name has beyond that run's, or so
///    at its end, e.g. "Bloom" for `gochuumon wa usagi desu ka ~bloom~`, "S"
///    for `bishoujo senshi sailor moon s`, or "Clannad: After Story" for
///    `clannad ~after story~`;
/// 2. else the one season left of as many episodes, e.g. "Season 2" for
///    `gochuumon wa usagi desu ka??`, once "Bloom" is taken;
/// 3. else the one season left, e.g. Mahoromatic's second for `mahoromatic
///    ~motto utsukushii mono~`, beside its first.
///
/// None if none tells, rather than a season its numbers would put it in
/// among another run's episodes.
pub(super) fn run_numbering(
    series: &SeriesRecord,
    run: &EpisodeRun,
    found_by: Option<&str>,
    [_, last]: [u32; 2],
) -> RunNumbering {
    let seasons = series.seasons.as_slice();
    let mut free: Vec<&SeasonSummary> = seasons.iter().filter(|season| season.number > 0).collect();
    free.sort_by_key(|season| season.number);
    let total: u32 = free.iter().map(|season| season.episode_count).sum();
    let named_after = run
        .family
        .iter()
        .filter(|(name, episodes)| {
            *name != run.name && run.name.starts_with(name.as_str()) && *episodes <= total
        })
        .min_by_key(|(name, _)| name.len())
        .map(|(name, _)| (name.as_str(), true));
    // Named as the series and a season, e.g. `jojos bizarre adventure -
    // golden wind`: that season, however the series was found.
    if named_after.is_none() {
        let run_words = title::word_list(&run.name);
        let named = [series.name.as_str(), series.original_name.as_str()]
            .into_iter()
            .filter_map(|name| {
                let own = run_words.strip_prefix(title::word_list(name).as_slice())?;
                let mut named = free.iter().filter(|season| {
                    !own.is_empty() && title::word_list(&season.name).ends_with(own)
                });
                match (named.next(), named.next()) {
                    (Some(season), None) => Some(season),
                    _ => None,
                }
            })
            .find(|season| season.episode_count == 0 || last <= season.episode_count);
        if let Some(season) = named {
            return RunNumbering::Season(Some(season.number));
        }
    }
    let by_found = found_by
        .filter(|name| title::begins_with(&run.name, name) && !title::same_words(&run.name, name))
        .map(|name| (name, false));
    let Some((first, in_release)) = named_after.or(by_found) else {
        return RunNumbering::Through;
    };
    if in_release && !free.is_empty() {
        free.remove(0);
    }
    let later: Vec<(&str, u32)> = match in_release {
        true => run
            .family
            .iter()
            .filter(|(name, _)| name != first && name.starts_with(first))
            .map(|(name, episodes)| (name.as_str(), *episodes))
            .collect(),
        false => vec![(run.name.as_str(), run.episodes)],
    };
    let first_words = title::word_list(first);
    let own_words = |name: &str| {
        let words = title::word_list(name);
        words
            .strip_prefix(first_words.as_slice())
            .map(|rest| rest.to_vec())
            .unwrap_or_default()
    };
    let mut taken = HashMap::<&str, u32>::new();
    // By name, each season to the one run named so.
    for &(name, _) in &later {
        let words = own_words(name);
        if words.is_empty() {
            continue;
        }
        let mut named = free
            .iter()
            .filter(|season| title::word_list(&season.name).ends_with(&words));
        if let (Some(season), None) = (named.next(), named.next()) {
            taken.insert(name, season.number);
        }
    }
    // Found by a name without words that are only a release's, e.g.
    // `remastered` or `720p eng sub`, which say nothing of a season.
    if !in_release
        && !taken.contains_key(run.name.as_str())
        && title::same_words(
            &release_words::without_weak(&release_words::cut(&run.name)),
            first,
        )
    {
        return RunNumbering::Through;
    }
    free.retain(|season| !taken.values().any(|&number| number == season.number));
    // By length, of what's left, until no more tells.
    loop {
        let found = later
            .iter()
            .filter(|(name, _)| !taken.contains_key(name))
            .find_map(|&(name, episodes)| {
                let mut as_long = free
                    .iter()
                    .filter(|season| season.episode_count == episodes);
                match (as_long.next(), as_long.next()) {
                    (Some(season), None) => Some((name, season.number)),
                    _ => None,
                }
            });
        let Some((name, number)) = found else {
            break;
        };
        taken.insert(name, number);
        free.retain(|season| season.number != number);
    }
    // The one season left, for the one run left.
    let left: Vec<&str> = later
        .iter()
        .map(|&(name, _)| name)
        .filter(|name| !taken.contains_key(name))
        .collect();
    if let ([name], [season]) = (left.as_slice(), free.as_slice()) {
        taken.insert(name, season.number);
    }
    RunNumbering::Season(
        taken
            .get(run.name.as_str())
            .and_then(|&number| seasons.iter().find(|season| season.number == number))
            .filter(|season| season.episode_count == 0 || last <= season.episode_count)
            .map(|season| season.number),
    )
}

/// The specials a file of a run of a release's episodes is, if the run goes
/// on past its series' first season by a few, fewer than the next has: the
/// specials that aired after the first season's last episode and before the
/// next's first, in the order they aired, e.g. Clannad's "The Events of
/// Summer Holidays" and "Another World: Tomoyo Chapter" for `Clannad 23` and
/// `24` of a season of 22, as a release may number the extras a season's
/// discs held after its episodes. Only if there are as many as the run goes
/// on by. Else, if a later run of the release is the next season, e.g.
/// `Clannad ~After Story~`, no episode at all, rather than one of that run's;
/// else none, and the file is numbered through the series as any other.
///
/// Some(None) for no episode; None for a file within its first season, or
/// of a run that goes on through the next.
pub(super) async fn past_first_season(
    kb: &dyn KnowledgeBaseApi,
    series: &ExternalId,
    run: &EpisodeRun,
    [first, last]: [u32; 2],
    kb_queries: &mut Vec<KbQueryEvent>,
) -> Result<Option<Option<[u32; 2]>>> {
    let Some(record) = kb.series(series, kb_queries).await? else {
        return Ok(None);
    };
    let mut numbered: Vec<&SeasonSummary> = record
        .seasons
        .iter()
        .filter(|season| season.number > 0)
        .collect();
    numbered.sort_by_key(|season| season.number);
    let [opening, next, ..] = numbered.as_slice() else {
        return Ok(None);
    };
    let ended = opening.episode_count;
    if first <= ended || run.last <= ended || run.last - ended >= next.episode_count {
        return Ok(None);
    }
    let extras = (run.last - ended) as usize;
    let (Some(opening_season), Some(next_season)) = (
        kb.season(series, opening.number, kb_queries).await?,
        kb.season(series, next.number, kb_queries).await?,
    ) else {
        return Ok(None);
    };
    let ended_on = opening_season.episodes.iter().filter_map(|e| e.aired).max();
    let began_on = next_season.episodes.iter().filter_map(|e| e.aired).min();
    if let (Some(ended_on), Some(began_on)) = (ended_on, began_on)
        && let Some(specials) = kb.season(series, 0, kb_queries).await?
    {
        let mut between: Vec<&Episode> = specials
            .episodes
            .iter()
            .filter(|special| {
                special
                    .aired
                    .is_some_and(|aired| ended_on < aired && aired < began_on)
            })
            .collect();
        between.sort_by_key(|special| (special.aired, special.number));
        if between.len() == extras {
            let at = |number: u32| between.get((number - ended - 1) as usize).map(|e| e.number);
            return Ok(Some(at(first).zip(at(last)).map(|(a, b)| [a, b])));
        }
    }
    let next_is_later_run = run
        .family
        .iter()
        .filter(|(name, _)| *name != run.name && name.starts_with(run.name.as_str()))
        .any(|(name, episodes)| {
            let later = EpisodeRun {
                name: name.clone(),
                episodes: *episodes,
                last: *episodes,
                family: run.family.clone(),
            };
            run_numbering(&record, &later, None, [1, 1]) == RunNumbering::Season(Some(next.number))
        });
    Ok(next_is_later_run.then_some(None))
}

/// The number of the season named like one of `names`, e.g. One Pace's season
/// 33 for a run named `Whole Cake Island`, unless it has fewer episodes than
/// `episodes` needs. A season the knowledge base knows no episodes of yet
/// may have any.
pub(super) fn season_named_like(
    seasons: &[SeasonSummary],
    names: &[&str],
    [_, last]: [u32; 2],
) -> Option<u32> {
    seasons
        .iter()
        .find(|season| {
            names
                .iter()
                .any(|name| title::same_words(&season.name, name))
                && (season.episode_count == 0 || last <= season.episode_count)
        })
        .map(|season| season.number)
}

/// How much a file's runtime may differ from a special's, in minutes, for
/// it to be that special, or a tenth of the special's if that's more.
pub(super) const SPECIAL_RUNTIME_TOLERANCE_MINUTES: f32 = 3.0;
