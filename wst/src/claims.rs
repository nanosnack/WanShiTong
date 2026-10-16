//! Claims as `wst` writes them, e.g. `movie_external_id tmdb:80809`: a kind,
//! as the JSON API names it, then its value.

use anyhow::{Context, anyhow, bail};
use wanshitong_api::v1::{Claim, ExternalId, FileRole, WorkKind};

/// The kinds of claim, for help and errors, wrapped for a terminal.
pub const KINDS: &str = "\
duration_seconds N, file_role main|extra|sample, movie_external_id ID,
movie_title TITLE, movie_release_year YEAR, tv_series_external_id ID,
tv_series_name NAME, tv_series_year YEAR, season N, episode_range N[-M],
absolute_episode_range N[-M], special, part N, special_of SERIES, extra_of PATH,
excluded, edition EDITION, episode_title TITLE,
work_kind movie|tv_series. An ID is tmdb:N, imdb:ttN or
tvdb:N.";

/// The help for `wst assert`, with [`KINDS`] in it, so that the two can't
/// disagree.
pub fn assert_about() -> String {
    format!(
        "\
Assert a claim about files, in an open changeset.

The claim is written as `wst` prints claims: its kind, then its value, e.g.
`movie_external_id tmdb:80809`, `movie_title \"Perry Mason Returns\"` or
`extra_of \"Heat (1995)/Heat.mkv\"`. It's pending until the changeset is
accepted, when it replaces any older assertion of the same kind about each
file.

Assert about all of a work's main files together, e.g. a movie's manifestations, so
they stay one work. Extras and disc files follow the file they're bound to.
Prints the new assertion's UUID.

Kinds of claim:
{KINDS}

A disc is named by its index file, e.g. `Film/BDMV/index.bdmv`, never its
other files."
    )
}

/// Parses a claim from its words, e.g. `["movie_title", "Heat"]`. A value of
/// several words is joined with spaces, and may be quoted as `wst` prints it.
pub fn parse_claim(words: &[String]) -> anyhow::Result<Claim> {
    let Some((kind, value)) = words.split_first() else {
        bail!("no claim; claims are:\n{KINDS}");
    };
    let value = value.join(" ");
    let value = value.trim();
    let text = || -> anyhow::Result<String> {
        let text = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        match text {
            "" => Err(anyhow!("{kind} needs a value")),
            text => Ok(text.to_owned()),
        }
    };
    let number = || -> anyhow::Result<u32> {
        value
            .parse()
            .with_context(|| format!("{kind} needs a number, not {value:?}"))
    };
    let range = || -> anyhow::Result<[u32; 2]> {
        let (first, last) = value.split_once('-').unwrap_or((value, value));
        let parse = |n: &str| {
            n.trim()
                .parse::<u32>()
                .with_context(|| format!("{kind} needs N or N-M, not {value:?}"))
        };
        Ok([parse(first)?, parse(last)?])
    };
    let claim = match kind.as_str() {
        "duration_seconds" => Claim::DurationSeconds(number()?.into()),
        "file_role" => Claim::FileRole(match value {
            "main" => FileRole::Main,
            "extra" => FileRole::Extra,
            "sample" => FileRole::Sample,
            _ => bail!("file_role is main, extra or sample, not {value:?}"),
        }),
        "movie_external_id" => Claim::MovieExternalId(parse_external_id(value)?),
        "movie_title" => Claim::MovieTitle(text()?),
        "movie_release_year" => Claim::MovieReleaseYear(number()?),
        "tv_series_external_id" => Claim::TvSeriesExternalId(parse_external_id(value)?),
        "tv_series_name" => Claim::TvSeriesName(text()?),
        "tv_series_year" => Claim::TvSeriesYear(number()?),
        "season" => Claim::Season(number()?),
        "episode_range" => Claim::EpisodeRange(range()?),
        "absolute_episode_range" => Claim::AbsoluteEpisodeRange(range()?),
        "special" if value.is_empty() => Claim::Special,
        "special" => bail!("special takes no value"),
        "part" => Claim::Part(number()?),
        "special_of" => Claim::SpecialOf(text()?),
        "extra_of" => Claim::ExtraOf(text()?),
        "excluded" if value.is_empty() => Claim::Excluded,
        "excluded" => bail!("excluded takes no value"),
        "edition" => Claim::Edition(text()?),
        "episode_title" => Claim::EpisodeTitle(text()?),
        "work_kind" => Claim::WorkKind(match value {
            "movie" => WorkKind::Movie,
            "tv_series" => WorkKind::TvSeries,
            _ => bail!("work_kind is movie or tv_series, not {value:?}"),
        }),
        _ => bail!("unknown kind of claim {kind:?}; claims are:\n{KINDS}"),
    };
    Ok(claim)
}

/// E.g. `tmdb:80809`, `imdb:tt0089801` or `tvdb:81189`.
fn parse_external_id(id: &str) -> anyhow::Result<ExternalId> {
    let parsed = match id.split_once(':') {
        Some(("tmdb", n)) => n.parse().ok().map(ExternalId::Tmdb),
        Some(("tvdb", n)) => n.parse().ok().map(ExternalId::Tvdb),
        Some(("imdb", n)) if n.starts_with("tt") => Some(ExternalId::Imdb(n.to_owned())),
        _ => None,
    };
    parsed.ok_or_else(|| anyhow!("an ID is tmdb:N, imdb:ttN or tvdb:N, not {id:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(claim: &str) -> anyhow::Result<Claim> {
        let words: Vec<String> = claim.split(' ').map(String::from).collect();
        parse_claim(&words)
    }

    #[test]
    fn parses_claims_as_wst_prints_them() {
        let cases = [
            (
                "movie_external_id tmdb:80809",
                Claim::MovieExternalId(ExternalId::Tmdb(80809)),
            ),
            (
                "tv_series_external_id imdb:tt0306414",
                Claim::TvSeriesExternalId(ExternalId::Imdb("tt0306414".into())),
            ),
            (
                "movie_title \"Perry Mason Returns\"",
                Claim::MovieTitle("Perry Mason Returns".into()),
            ),
            ("movie_title Heat", Claim::MovieTitle("Heat".into())),
            (
                "edition \"Director's Cut\"",
                Claim::Edition("Director's Cut".into()),
            ),
            ("episode_range 1-2", Claim::EpisodeRange([1, 2])),
            ("episode_range 3", Claim::EpisodeRange([3, 3])),
            ("file_role extra", Claim::FileRole(FileRole::Extra)),
            ("special", Claim::Special),
            (
                "extra_of Heat (1995)/Heat.mkv",
                Claim::ExtraOf("Heat (1995)/Heat.mkv".into()),
            ),
        ];
        for (text, claim) in cases {
            assert_eq!(parse(text).unwrap(), claim, "{text}");
        }
    }

    #[test]
    fn rejects_what_isnt_a_claim() {
        for text in [
            "",
            "movie_title",
            "movie_external_id 80809",
            "imdb:0089801 movie_external_id",
            "season one",
            "file_role feature",
            "special 1",
            "made_up 1",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
