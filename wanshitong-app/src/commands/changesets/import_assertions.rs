use crate::commands::{add_assertions, create_changeset, discard_changeset};
use crate::{AppContext, AppError, AppErrorContext, report_error};
use jiff::Timestamp;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use wanshitong_core::commands::{NewAssertion, NewChangeset};
use wanshitong_core::domain::{
    AssertionUuid, ChangesetUuid, ClaimPredicate, DataSourceUuid, PathId,
};
use wanshitong_core::queries::list_data_sources;

/// Assertions exported from an instance of WST, e.g. a backup of this one's,
/// to make again here.
pub struct AssertionImport {
    /// Who the changeset is by, e.g. "import".
    pub author: String,
    /// What it's for, e.g. which file it came from.
    pub description: String,
    pub assertions: Vec<ImportedAssertion>,
}

pub struct ImportedAssertion {
    pub files: Vec<ImportedFile>,
    pub claim: ClaimPredicate,
    pub rationale: Option<String>,
    pub confidence: Option<f32>,
    /// When it was made where it came from, to make them in the same order.
    pub created_at: Timestamp,
}

/// A file as the instance it came from named it: by its data source there,
/// whose UUID and name may or may not be one of ours.
pub struct ImportedFile {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
}

/// The changeset an import made, and the assertions in it, in the order they
/// were made.
#[derive(Debug)]
pub struct ImportedAssertions {
    pub changeset: ChangesetUuid,
    pub assertions: Vec<AssertionUuid>,
}

/// Makes the assertions in a new changeset, left open to review or accept:
/// all or none of them, oldest first, so that they keep their order. Each
/// file's data source is ours with its UUID, if there is one, and otherwise
/// ours with its name, since another instance's UUIDs mean nothing here.
/// Fails naming every data source that's missing, before anything is made.
pub async fn import_assertions(
    ctx: &AppContext,
    import: AssertionImport,
) -> Result<ImportedAssertions, AppError> {
    let AssertionImport {
        author,
        description,
        assertions: mut imported,
    } = import;
    if imported.is_empty() {
        return Err(AppErrorContext::NothingToImport.into());
    }
    let sources = list_data_sources(&ctx.core).await?;
    let uuids: HashSet<DataSourceUuid> = sources.iter().map(|source| source.uuid).collect();
    let by_name: HashMap<&str, DataSourceUuid> = sources
        .iter()
        .map(|source| (source.name.as_str(), source.uuid))
        .collect();
    // Stable, so those made together keep the order they came in.
    imported.sort_by_key(|assertion| assertion.created_at);

    let mut missing = BTreeSet::new();
    let assertions: Vec<NewAssertion> = imported
        .into_iter()
        .map(
            |ImportedAssertion {
                 files,
                 claim,
                 rationale,
                 confidence,
                 created_at: _,
             }| NewAssertion {
                files: files
                    .into_iter()
                    .filter_map(
                        |ImportedFile {
                             data_source_uuid,
                             data_source_name,
                             rel_path,
                         }| {
                            let data_source = if uuids.contains(&data_source_uuid) {
                                data_source_uuid
                            } else if let Some(&uuid) = by_name.get(data_source_name.as_str()) {
                                uuid
                            } else {
                                missing.insert(data_source_name);
                                return None;
                            };
                            Some(PathId {
                                data_source,
                                rel_path,
                            })
                        },
                    )
                    .collect(),
                claim,
                rationale,
                confidence,
            },
        )
        .collect();
    if !missing.is_empty() {
        return Err(AppErrorContext::DataSourcesNotFound(missing.into_iter().collect()).into());
    }

    let changeset = create_changeset(
        ctx,
        NewChangeset {
            author,
            description,
        },
    )
    .await?;
    match add_assertions(ctx, changeset, assertions).await {
        Ok(assertions) => Ok(ImportedAssertions {
            changeset,
            assertions,
        }),
        Err(error) => {
            // None of them were made, so the changeset has nothing to keep.
            if let Err(discard_error) = discard_changeset(ctx, changeset).await {
                report_error!(
                    discard_error,
                    %changeset,
                    "Couldn't discard the changeset of a failed import"
                );
            }
            Err(error)
        }
    }
}
