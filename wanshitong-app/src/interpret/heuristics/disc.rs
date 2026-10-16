//! Heuristics for discs, and the files ripping one leaves behind.

use super::{
    HeuristicView, movie_directory::title_claims_from_dir, names::MOVIE_TITLE_REGEX,
    series_patterns::SeriesPatterns,
};
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, DiscKind, FileRole, Heuristic, RankedClaimPredicate, SubjectIndex,
    is_disc_image,
};

/// Makes claims about each disc, which its index file stands for (see *Disc*
/// in `spec/domain-model.md`): the title and year in its folder's name, that
/// it's its movie's main file, and its longest title's duration, as ffprobe
/// can't read an index file.
///
/// A disc of a set is named by the set's folder, if that names anything: a
/// disc in a folder named for its number, e.g. `D1` or `Disc 2`, or one of
/// several in folders named alike but for their numbers, e.g. `SHOW_D1` and
/// `SHOW_D2`, `CLANNAD_1` and `CLANNAD_2`, or `Vol 1 [01-04]` and
/// `Vol 2 [05-08]`. Its own folder's name is looked up too, below the set's,
/// for a set of movies, e.g. `Kill Bill Vol 1` and `Vol 2`. A season
/// release's disc, e.g. one in `Show.Name.S01.1080i.Blu-ray/D1`, is its
/// series': it claims the series' name rather than a movie's title. A set
/// of three discs or more, without a season, is as likely a series' as a
/// movie's, so its name is looked up as a series' too, below the rest.
pub(super) fn disc_heuristic(
    view: HeuristicView,
    claims: &mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    rank: f32,
) {
    let movie_title_re = Regex::new(MOVIE_TITLE_REGEX).unwrap();
    let disc_number_re = Regex::new(DISC_NUMBER_REGEX).unwrap();
    let disc_label_re = Regex::new(DISC_LABEL_REGEX).unwrap();
    let volume_re = Regex::new(VOLUME_LABEL_REGEX).unwrap();
    let digits_re = Regex::new(r"\d+").unwrap();
    let series_patterns = SeriesPatterns::new();
    // A disc's folder, as it names the disc: a disc image is named like a
    // folder, e.g. `Movie (2001).iso` like `Movie (2001)`, or `D1.iso` in a
    // set's folder.
    let root_of = |rel_path: &Path, root: &str| {
        if is_disc_image(&rel_path.to_string_lossy()) {
            Path::new(root).with_extension("")
        } else {
            PathBuf::from(root)
        }
    };
    // A set's key: the folder the disc's folder is in, and the disc folder's
    // name with its numbers masked, if it has any.
    let set_key = |root: &Path| {
        let name = root.file_name()?.to_str()?;
        let parent = root
            .parent()
            .filter(|parent| parent.file_name().is_some())?;
        digits_re.is_match(name).then(|| {
            (
                parent.to_path_buf(),
                digits_re.replace_all(name, "#").into_owned(),
            )
        })
    };
    let mut set_sizes = HashMap::<(PathBuf, String), usize>::new();
    for (path_id, facts) in view.files {
        if let Some(disc) = &facts.disc
            && let Some(key) = set_key(&root_of(&path_id.rel_path, &disc.root))
        {
            *set_sizes.entry(key).or_default() += 1;
        }
    }
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let Some(disc) = &facts.disc else {
            continue;
        };
        let source = ClaimSource::Heuristic(match disc.kind {
            DiscKind::BluRay => Heuristic::Bdmv,
            DiscKind::Dvd => Heuristic::Dvd,
        });
        let root = root_of(&path_id.rel_path, &disc.root);
        let root = root.as_path();
        let set_size = set_key(root).and_then(|key| set_sizes.get(&key).copied());
        let folder_name = root.file_name().and_then(|name| name.to_str());
        // A folder that names nothing of its own: one named for the disc's
        // number, or a disc's generic label, e.g. `BDROM`.
        let named_for_its_number = folder_name
            .is_some_and(|name| disc_number_re.is_match(name) || disc_label_re.is_match(name));
        // A volume's label, e.g. `CHEER FRUITS VOL1`: a disc of a set, though
        // the only one here.
        let volume = folder_name.is_some_and(|name| volume_re.is_match(name));
        let set = root
            .parent()
            .filter(|parent| parent.file_name().is_some())
            .filter(|_| named_for_its_number || volume || set_size.is_some_and(|size| size > 1));
        // What a folder's name says: the series of a season release, or else a
        // movie's title and year.
        let name_claims = |dir: &Path, rank: f32| {
            let series = dir
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| series_patterns.season_release_series(name));
            match series {
                Some(series) => vec![RankedClaimPredicate::new(
                    source,
                    rank,
                    ClaimPredicate::TvSeriesName(series),
                )],
                None => title_claims_from_dir(&movie_title_re, dir, source, rank),
            }
        };
        let disc_claims = claims.entry(subject).or_default();
        // The set's folder, unless it names nothing, e.g. one named only in
        // tags, `[BDMV][Title][BOX]`: then the disc's own.
        match set
            .map(|set| name_claims(set, rank))
            .filter(|c| !c.is_empty())
        {
            Some(set_claims) => {
                let movie_titles: Vec<String> = set_claims
                    .iter()
                    .filter_map(|claim| match &claim.pred {
                        ClaimPredicate::MovieTitle(title) => Some(title.clone()),
                        _ => None,
                    })
                    .collect();
                disc_claims.extend(set_claims);
                // Not a folder named for its number, e.g. `Disc 1`, which
                // names nothing.
                if !named_for_its_number {
                    disc_claims.extend(name_claims(root, rank - ALTERNATIVE_STEP));
                }
                if set_size.unwrap_or(0) >= 3 {
                    disc_claims.extend(movie_titles.into_iter().map(|title| {
                        RankedClaimPredicate::new(
                            source,
                            rank - 2.0 * ALTERNATIVE_STEP,
                            ClaimPredicate::TvSeriesName(title),
                        )
                    }));
                }
            }
            None => disc_claims.extend(name_claims(root, rank)),
        }
        // A whole disc is its movie's main file.
        disc_claims.push(RankedClaimPredicate::new(
            source,
            rank,
            ClaimPredicate::FileRole(FileRole::Main),
        ));
        if let Some(duration) = disc.duration {
            disc_claims.push(RankedClaimPredicate::new(
                source,
                rank,
                ClaimPredicate::Duration(duration),
            ));
        }
    }
}

/// How far below the name a disc's set gives it the other names it could
/// have rank: its own folder's, then its set's as a series'.
const ALTERNATIVE_STEP: f32 = 1.0;

/// Excludes files under MAKEMKV/CMAP/ directories.
pub(super) fn makemkv_cmap_heuristic(view: HeuristicView, exclusions: &mut HashSet<SubjectIndex>) {
    for (subject, (path_id, _)) in view.files.iter().enumerate() {
        if is_makemkv_cmap_path(&path_id.rel_path) {
            exclusions.insert(subject);
        }
    }
}

fn is_makemkv_cmap_path(path: &Path) -> bool {
    let mut found_makemkv = false;
    for component in path.components() {
        let name = component.as_os_str();
        if found_makemkv && name.eq_ignore_ascii_case("CMAP") {
            return true;
        }
        found_makemkv = name.eq_ignore_ascii_case("MAKEMKV");
    }
    false
}

/// A disc's generic label, which names nothing, e.g. `BDROM`, `BD_VIDEO` or
/// `BDMV`, as a disc's folder is often named by its volume label.
const DISC_LABEL_REGEX: &str =
    r"(?i)^(?:bd[\s._-]?rom|bd[\s._-]?video|bdmv|disc|disk|video|dvd[\s._-]?video|blu-?ray)$";

/// A volume's label, ending in its number, e.g. `CHEER FRUITS VOL1`,
/// `SHOW_DISC_8` or `SHOW D2`.
const VOLUME_LABEL_REGEX: &str =
    r"(?i)[\s._-](?:vol(?:ume)?|dis[ck]|d)[\s._-]*\d{1,2}$|vol\d{1,2}$";

/// A folder named for a disc's number in its set, e.g. `D1`, `Disc 2`,
/// `CD1` or `DVD_3`.
const DISC_NUMBER_REGEX: &str = r"(?i)^(?:d|dis[ck]|cd|bd|dvd)[\s._-]*\d{1,2}$";
