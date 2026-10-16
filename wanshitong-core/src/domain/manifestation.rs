use std::path::Path;

/// Where a movie's main file is, and which part of its manifestation it is,
/// as its manifestation is told by. See [`manifestations_of`], and
/// *Manifestation* in `spec/domain-model.md`.
#[derive(Clone, Copy, Debug)]
pub struct MainFileLocation<'a, D> {
    pub data_source: D,
    /// Relative to the data source's root.
    pub rel_path: &'a Path,
    /// The file's device and inode, if they were measured.
    pub device_inode: Option<(u64, u64)>,
    /// Which part of its manifestation the file is, if its name numbers it
    /// as one, e.g. 1 for `CD1` or `Part One`.
    pub part: Option<u32>,
}

/// Which manifestation of their movie each of its main files is, as numbers
/// from 0 in the order of the manifestations' first files. Each main file is
/// a manifestation of its own, unless something says two are one: paths to
/// the same file, or parts of one manifestation, which are main files in one
/// directory that are numbered as different parts. Of several parts numbered
/// alike beside each other, e.g. two `CD1`s, each goes with the first
/// manifestation, by its files' paths, that has no part of that number yet.
pub fn manifestations_of<D: Eq>(files: &[MainFileLocation<D>]) -> Vec<usize> {
    // Each file starts in a manifestation of its own, labeled by its index,
    // then joins that of a file it's one manifestation with, and that
    // manifestation's files join it too.
    let mut label: Vec<usize> = (0..files.len()).collect();
    fn join(label: &mut [usize], a: usize, b: usize) {
        let (from, to) = (label[a].max(label[b]), label[a].min(label[b]));
        for l in label.iter_mut().filter(|l| **l == from) {
            *l = to;
        }
    }
    for i in 0..files.len() {
        for j in 0..i {
            let same_file =
                files[i].device_inode.is_some() && files[i].device_inode == files[j].device_inode;
            if same_file && label[i] != label[j] {
                join(&mut label, i, j);
            }
        }
    }

    // The parts, in the order of their paths. Each joins the first
    // manifestation of parts beside it that lacks its part number, if any.
    let mut parts: Vec<usize> = (0..files.len())
        .filter(|&i| files[i].part.is_some())
        .collect();
    parts.sort_by_key(|&i| files[i].rel_path);
    for (k, &i) in parts.iter().enumerate() {
        let beside = |j: usize| {
            files[j].data_source == files[i].data_source
                && files[j].rel_path.parent() == files[i].rel_path.parent()
        };
        // The manifestations of the parts beside this one, each with its
        // parts' numbers, in the order of their first parts.
        let mut beside_parts: Vec<(usize, Vec<Option<u32>>)> = Vec::new();
        for &j in parts[..k].iter().filter(|&&j| beside(j)) {
            match beside_parts.iter_mut().find(|(l, _)| *l == label[j]) {
                Some((_, numbers)) => numbers.push(files[j].part),
                None => beside_parts.push((label[j], vec![files[j].part])),
            }
        }
        if let Some(&(l, _)) = beside_parts
            .iter()
            .find(|(_, numbers)| !numbers.contains(&files[i].part))
        {
            let j = label
                .iter()
                .position(|&other| other == l)
                .expect("a manifestation has files");
            if label[i] != label[j] {
                join(&mut label, i, j);
            }
        }
    }

    // Number the manifestations densely, in the order of their first files.
    let mut numbers = Vec::new();
    label
        .iter()
        .map(|l| match numbers.iter().position(|n| n == l) {
            Some(number) => number,
            None => {
                numbers.push(*l);
                numbers.len() - 1
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(data_source: u8, rel_path: &str, inode: Option<u64>) -> MainFileLocation<'_, u8> {
        MainFileLocation {
            data_source,
            rel_path: Path::new(rel_path),
            device_inode: inode.map(|inode| (1, inode)),
            part: None,
        }
    }

    fn part(data_source: u8, rel_path: &str, part: u32) -> MainFileLocation<'_, u8> {
        MainFileLocation {
            part: Some(part),
            ..at(data_source, rel_path, None)
        }
    }

    #[test]
    fn each_main_file_is_a_manifestation_unless_paths_to_one_file() {
        let files = [
            at(1, "Alien.3.Assembly.Cut.1992.REMUX.mkv", Some(10)),
            // Two encodes in one directory, nothing numbering them as parts.
            at(1, "Alien (1992)/Alien (1992) fixed.mkv", Some(11)),
            at(1, "Alien (1992)/Alien (1992) tv-dx50.mkv", Some(12)),
            // The first file again, by a hard link elsewhere.
            at(2, "Collection/Alien 3.mkv", Some(10)),
            // Not measured, so no other path is known to be the same file.
            at(3, "Alien 3.mkv", None),
            at(3, "Alien III.mkv", None),
        ];
        assert_eq!(manifestations_of(&files), vec![0, 1, 2, 0, 3, 4]);
    }

    #[test]
    fn parts_numbered_differently_in_one_directory_are_one_manifestation() {
        let files = [
            part(1, "Alien (1992) 2160p/Alien (1992) - CD2.mkv", 2),
            part(1, "Alien (1992) 2160p/Alien (1992) - CD1.mkv", 1),
            // The same parts in another directory, and in another data source.
            part(1, "Alien (1992) 1080p/Alien (1992) - CD1.mkv", 1),
            part(1, "Alien (1992) 1080p/Alien (1992) - CD2.mkv", 2),
            part(2, "Alien (1992) 2160p/Alien (1992) - CD1.mkv", 1),
            // Not a part, so not of the parts beside it.
            at(1, "Alien (1992) 2160p/Alien (1992) - Whole.mkv", None),
        ];
        assert_eq!(manifestations_of(&files), vec![0, 0, 1, 1, 2, 3]);
    }

    #[test]
    fn parts_numbered_alike_go_with_the_first_manifestation_without_that_part() {
        // Two sets of parts in one directory, each with a `CD1` and a `CD2`.
        let files = [
            part(1, "Alien/Alien 1080p CD1.mkv", 1),
            part(1, "Alien/Alien 1080p CD2.mkv", 2),
            part(1, "Alien/Alien 720p CD1.mkv", 1),
            part(1, "Alien/Alien 720p CD2.mkv", 2),
        ];
        // In path order, the 1080p parts make one manifestation, then
        // `720p CD1` finds it has a part 1 and starts another, which
        // `720p CD2` joins.
        assert_eq!(manifestations_of(&files), vec![0, 0, 1, 1]);
    }

    #[test]
    fn a_hard_link_to_a_part_is_of_its_manifestation() {
        let files = [
            MainFileLocation {
                device_inode: Some((1, 1)),
                ..part(1, "Release/a.cd1.mkv", 1)
            },
            at(2, "Elsewhere/a.mkv", Some(1)),
            part(1, "Release/a.cd2.mkv", 2),
        ];
        assert_eq!(manifestations_of(&files), vec![0, 0, 0]);
    }
}
