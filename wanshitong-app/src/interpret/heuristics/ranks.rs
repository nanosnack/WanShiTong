//! The ranks of the heuristics' claims, highest first, and of a knowledge
//! base result's claims about the file rather than the work.
//!
//! When claims about a file conflict, the higher-ranked wins (see *Belief
//! Policy* in `spec/domain-model.md`), so the ranks are totally ordered: no
//! two heuristics whose claims can conflict share one. Assertions outrank
//! every heuristic, and what a knowledge base finds by a claim is ranked a
//! little above it, by the bonuses in `knowledge_base::search`, as it
//! competes with other identifications. A result's claims about the file,
//! e.g. that it's as long as the movie, so its main file, compete with the
//! heuristics' only once the result stands, so they're ranked here, as that
//! kind of evidence (see [`Rank::fixed`](wanshitong_core::domain::Rank::fixed)).
//!
//! `run_narrow_phase_heuristics` runs the heuristics in another order than
//! this, as some go by the claims before them.

/// Being in an extras directory outranks what knowledge bases say about a
/// file on its own; only assertions outrank it.
pub(super) const EXTRAS_DIRECTORY: f32 = 1000.0;
/// An ID in an NFO file outranks one written in a name: a media server's
/// manager writes the NFO for the work it identified, where a name's ID may
/// be a directory's above, e.g. a series' for the movies in it.
pub(super) const NFO_EXTERNAL_ID: f32 = 210.0;
/// An ID written in a name outranks what a title finds, as anything a
/// knowledge base finds by one does, but not an assertion.
pub(super) const NAME_EXTERNAL_ID: f32 = 200.0;
pub(super) const DISC: f32 = 100.0;
/// Outranks the roles from the movie directory, so that a sample is excluded
/// even when it is the only video in its movie directory.
pub(super) const SAMPLE: f32 = 60.0;
/// Outranks the series a video's own name gives, as the release it's in says
/// whose it is.
pub(super) const SEASON_RELEASE: f32 = 59.5;
/// Above the series a video's own name gives, which is one of them, so that
/// both are looked up.
pub(super) const SERIES_ALSO_KNOWN_AS: f32 = 59.3;

// Evidence that a video is an episode, from what its name and directory say
// explicitly, down to numbered runs of its siblings. It outranks the movie
// titles below, which an episode's file name also parses as, and a movie
// directory's title, as a directory of episodes isn't a movie's. And the
// movies a knowledge base finds by those titles, unless something corroborates
// their runtime matching the video's, e.g. the year. Below a Sample claim, so
// that an episode's sample is left out.
pub(super) const FILE_NAME_SERIES_NAME: f32 = 59.0;
/// Below the name before a file's explicit marker. Either may be the
/// series', and a knowledge base finding one outranks both.
pub(super) const SERIES_NAME_BEFORE_SEASON_EPISODE_NUMBER: f32 = 58.7;
/// Below the series an extra's own name says it's of.
pub(super) const SERIES_OF_NEARBY_EPISODES: f32 = 58.5;
pub(super) const FILE_NAME_SEASON_EPISODE: f32 = 58.0;
/// Just below the file name's explicit marker: an NFO's season and episode
/// were scraped from the name, usually, and an NFO left over from a file
/// renamed since is stale where the name isn't.
pub(super) const NFO_EPISODE: f32 = 57.9;
/// Above an episode without a season, as the directory corroborates this
/// one's season, where it only assumes the other's episode is numbered within
/// it, e.g. Perry Mason's `515` and `E138`, which is numbered through the
/// whole series.
pub(super) const SEASON_EPISODE_NUMBER: f32 = 57.8;
/// A number through the series beside a season's episode number, which only
/// a knowledge base goes by, to number the file as it does.
pub(super) const EPISODE_IN_PARENTHESES: f32 = 57.6;
pub(super) const FILE_NAME_EPISODE: f32 = 57.5;
pub(super) const SEASON_DIRECTORY: f32 = 57.0;
/// Above a movie directory's title, as the series searched for by it, e.g.
/// for `S02OVA` alone in a release's directory, outranks that title.
pub(super) const SERIES_SPECIAL: f32 = 56.0;
/// A numbered run of videos that aren't feature length.
pub(super) const EPISODE_RUN: f32 = 55.0;
/// An NFO's title, year and kind of work: a media server's manager wrote the
/// work's, where a directory is named after the release. Below the evidence
/// that a video is an episode, which an NFO's `<movie>` can't make a movie.
pub(super) const NFO_WORK: f32 = 51.0;

pub(super) const MOVIE_DIRECTORY: f32 = 50.0;
pub(super) const CONTAINER_DURATION: f32 = 10.0;
/// A run of feature-length videos is more likely a series' movies, e.g.
/// `Star Wars - 01 - The Phantom Menace`, so it's weak evidence.
pub(super) const FEATURE_RUN: f32 = 6.7;
/// Below the claims that a file is a movie, as its knowledge base looks it up
/// as a special only when its titles find no movie.
pub(super) const SPECIAL_OF_NEARBY_SERIES: f32 = 5.0;
/// Above a part's number, as a bonus video numbered in a tag, e.g.
/// `[PV][01]`, is one of several, not a part of the movie.
pub(super) const EXTRAS_OF_NEARBY_MOVIE: f32 = 4.5;
pub(super) const FILE_NAME_PART: f32 = 4.2;
/// Below a part the name says in words, e.g. `Part 2`, as a number in a tag
/// says it only by its siblings'.
pub(super) const PART_IN_TAG: f32 = 4.1;
pub(super) const RELEASE_NAME: f32 = 4.0;
/// Below the evidence that a video is a movie's main file, from its release
/// name or its movie directory, so that a movie titled like an extra, e.g.
/// `Happy Endings (2005)`, stays the main file. Above anything a knowledge
/// base says about the role.
pub(super) const EXTRAS_NAME_OR_DIRECTORY: f32 = 3.8;
/// A knowledge base's claim that a file is a movie's main file, for lasting
/// as long as the movie. Below an extra's name or directory, which say what
/// a file is where its length doesn't, e.g. a featurette as long as a
/// short. It needs no more than that, as it's the only evidence of a role
/// where nothing in the file's name is.
pub(crate) const KB_FILE_ROLE: f32 = 3.5;
/// A number set apart in a lone video's name, e.g. `Show - 19`: above its
/// name taken for a movie's title, below a knowledge base's movie as long
/// as it, whose main role outranks it.
pub(super) const LONE_NUMBERED_EPISODE: f32 = 3.3;
/// Only ever conflicts with another title, e.g. a knowledge base's.
/// Only ever conflicts with another kind.
pub(super) const KIND_OF_WORK: f32 = 3.2;
/// Below a video's kind by its name: a duration only says it's long.
pub(super) const KIND_OF_DURATION: f32 = 3.18;
/// Below a video's own kind: its neighbours' says less.
pub(super) const KIND_OF_SIBLINGS: f32 = 3.15;
pub(super) const FILE_NAME_EPISODE_TITLE: f32 = 3.1;
/// Only ever conflicts with another edition.
pub(super) const FILE_NAME_EDITION: f32 = 3.0;
/// A knowledge base's edition, from an entry it has for a cut of a movie,
/// e.g. `Kingdom Of Heaven (Director's Cut)`. Below the file name's, which
/// is of the file, and may say more, e.g. `Director's Cut Roadshow`.
pub(crate) const KB_EDITION: f32 = 2.9;
pub(super) const CONTAINER_TITLE_TAG: f32 = 2.0;
pub(super) const FILE_NAME_TITLE: f32 = 1.0;
pub(super) const TITLE_WITHOUT_NUMBERING: f32 = 0.5;
/// Below the file name's title too, as the part is sometimes the title's,
/// e.g. `Dune Part Two`.
pub(super) const TITLE_BEFORE_PART: f32 = 0.4;
/// Below the file name's other titles, like the title without release noise,
/// as an edition's words are sometimes a title's.
pub(super) const TITLE_WITHOUT_EDITION: f32 = 0.35;
/// Below the file name's other titles, as what's taken for release noise is
/// sometimes a title's word.
pub(super) const TITLE_WITHOUT_RELEASE_NOISE: f32 = 0.3;
/// Below a series' name it's cut from, as a release's words are sometimes a
/// series' own, e.g. a "Remastered" edition TMDB lists apart.
pub(super) const SERIES_NAME_WITHOUT_RELEASE_WORDS: f32 = 0.25;
/// The lowest: a title cut before its subtitle is looked up only if nothing
/// else finds a work.
pub(super) const TITLE_BEFORE_SUBTITLE: f32 = 0.2;
/// As low: a title split at its capitals is looked up only if nothing else
/// finds a work, after one cut before its subtitle.
/// As low: a title without a group's tag it begins with is looked up only if
/// nothing else finds a work, after one cut before its subtitle.
/// As low: a title cut to one word is looked up only if nothing else finds a
/// work, after one cut before its subtitle.
pub(super) const TITLE_OF_ONE_WORD: f32 = 0.17;
pub(super) const TITLE_WITHOUT_GROUP_TAG: f32 = 0.15;
pub(super) const TITLE_SPLIT_AT_CAPITALS: f32 = 0.1;
/// As low: a title with long vowels written short is looked up only if
/// nothing else finds a work, after those cut or split.
pub(super) const TITLE_WITH_SHORT_VOWELS: f32 = 0.05;
/// As low: a title without `and` is looked up only if nothing else finds a
/// work, after those cut, split or spelled with short vowels.
pub(super) const TITLE_WITHOUT_AND: f32 = 0.03;

#[cfg(test)]
mod tests {
    use super::*;

    /// The table above, in its order. A new rank goes in both.
    const HIGHEST_FIRST: [f32; 49] = [
        EXTRAS_DIRECTORY,
        NFO_EXTERNAL_ID,
        NAME_EXTERNAL_ID,
        DISC,
        SAMPLE,
        SEASON_RELEASE,
        SERIES_ALSO_KNOWN_AS,
        FILE_NAME_SERIES_NAME,
        SERIES_NAME_BEFORE_SEASON_EPISODE_NUMBER,
        SERIES_OF_NEARBY_EPISODES,
        FILE_NAME_SEASON_EPISODE,
        NFO_EPISODE,
        SEASON_EPISODE_NUMBER,
        EPISODE_IN_PARENTHESES,
        FILE_NAME_EPISODE,
        SEASON_DIRECTORY,
        SERIES_SPECIAL,
        EPISODE_RUN,
        NFO_WORK,
        MOVIE_DIRECTORY,
        CONTAINER_DURATION,
        FEATURE_RUN,
        SPECIAL_OF_NEARBY_SERIES,
        EXTRAS_OF_NEARBY_MOVIE,
        FILE_NAME_PART,
        PART_IN_TAG,
        RELEASE_NAME,
        EXTRAS_NAME_OR_DIRECTORY,
        KB_FILE_ROLE,
        LONE_NUMBERED_EPISODE,
        KIND_OF_WORK,
        KIND_OF_DURATION,
        KIND_OF_SIBLINGS,
        FILE_NAME_EPISODE_TITLE,
        FILE_NAME_EDITION,
        KB_EDITION,
        CONTAINER_TITLE_TAG,
        FILE_NAME_TITLE,
        TITLE_WITHOUT_NUMBERING,
        TITLE_BEFORE_PART,
        TITLE_WITHOUT_EDITION,
        TITLE_WITHOUT_RELEASE_NOISE,
        SERIES_NAME_WITHOUT_RELEASE_WORDS,
        TITLE_BEFORE_SUBTITLE,
        TITLE_OF_ONE_WORD,
        TITLE_WITHOUT_GROUP_TAG,
        TITLE_SPLIT_AT_CAPITALS,
        TITLE_WITH_SHORT_VOWELS,
        TITLE_WITHOUT_AND,
    ];

    #[test]
    fn ranks_are_distinct_and_listed_highest_first() {
        for pair in HIGHEST_FIRST.windows(2) {
            assert!(pair[0] > pair[1], "{} is not above {}", pair[0], pair[1]);
        }
    }
}
