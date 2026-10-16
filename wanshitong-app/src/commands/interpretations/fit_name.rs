use wanshitong_core::MAX_NAME_LEN;

/// Truncates an interpretation name to fit the name length limit.
pub(crate) fn fit_name(name: String) -> String {
    if name.len() <= MAX_NAME_LEN {
        return name;
    }
    const ELLIPSIS: char = '…';
    let mut end = MAX_NAME_LEN - ELLIPSIS.len_utf8();
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", name[..end].trim_end())
}
