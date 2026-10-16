//! A data source's files. The page is the file's, in `rel_path`.

pub mod rel_path;

use topcoat::router::path_param;
use uuid::Uuid;

path_param!(pub source_id: Uuid, error = not_found);
