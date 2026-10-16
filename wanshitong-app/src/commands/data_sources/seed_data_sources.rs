use crate::commands::{create_data_source, start_ingest};
use crate::queries::{list_data_sources, list_deleted_data_sources};
use crate::report_error;
use crate::{AppContext, AppError, DeclaredDataSource};
use tracing::{info, warn};
use wanshitong_core::domain::{DataSourceConfig, FileSystemSource};

/// Creates the config file's data sources that no data source, even a deleted
/// one, is named for, and starts ingesting each. Returns how many it created.
/// One that already exists is left alone, even if the config differs, so that
/// a fresh instance can be set up from config while the UI stays in charge.
pub async fn seed_data_sources(ctx: &AppContext) -> Result<usize, AppError> {
    let declared = &ctx.config().data_sources;
    if declared.is_empty() {
        return Ok(0);
    }
    let mut existing = list_data_sources(ctx).await?;
    existing.extend(list_deleted_data_sources(ctx).await?);

    let mut created = 0;
    for DeclaredDataSource {
        name,
        root_path,
        follow_symlinks,
    } in declared
    {
        let config = DataSourceConfig::FileSystem(FileSystemSource {
            root_path: root_path.clone(),
            follow_symlinks: *follow_symlinks,
        });
        if let Some(source) = existing.iter().find(|s| s.name == name.trim()) {
            if source.config != config {
                warn!(
                    "Data source {:?} differs from the config file's; leaving it as it is",
                    source.name
                );
            }
            continue;
        }
        let source = create_data_source(ctx, name, config).await?;
        info!("Created data source {:?} from the config file", source.name);
        created += 1;
        // The source exists either way; an ingest can be started from the UI.
        if let Err(e) = start_ingest(ctx, source.uuid).await {
            report_error!(e, data_source = ?source.name, "Couldn't start ingesting");
        }
    }
    Ok(created)
}
