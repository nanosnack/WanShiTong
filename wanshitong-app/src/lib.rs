#![doc = include_str!("../README.md")]

pub mod commands;
pub mod corpus;
pub mod ingest;
pub mod interpretations;
pub mod onboarding;
pub mod queries;
pub mod views;

mod config;
mod context;
mod data_source_types;
mod edition;
mod error;
mod expiring_cache;
mod file_probe;
mod http_response_ext;
mod interpret;
mod knowledge_base_types;
mod title;
mod tmdb_client;

pub use config::{Config, DeclaredDataSource, SHARED_TMDB_TOKEN};
pub use context::AppContext;
#[doc(hidden)]
pub use error::__private;
pub use error::{AppError, AppErrorContext};

// Minimal re-exports from core.
//
// The goal is for users of the app layer to avoid using core directly for clean
// layering.
//
pub use wanshitong_core::{CoreContext, CoreError, CoreErrorContext};
pub mod domain {
    pub use wanshitong_core::domain::{
        AssertedFile, Assertion, AssertionUuid, CatalogFile, CatalogWork, CatalogWorkKind,
        ChangesetDetails, ChangesetState, ChangesetUuid, ClaimPredicate, ClaimSource, DataSource,
        DataSourceConfig, DataSourceRef, DataSourceUuid, DirectoryFile, DiscFacts, DiscKind,
        DiscSummary, ExternalId, FileDetails, FileEpisode, FileMediaMeasurements, FileMovie,
        FilePart, FileRole, FileSeriesExtra, FileSummary, FileSystemSource, Heuristic,
        IngestFailureStage, IngestFileFailure, Interpretation, InterpretationChangeset,
        InterpretationError, InterpretationIngest, InterpretationUuid, JellyfinView,
        KbLookupResult, KbQueryEvent, KbSearchResult, Mark, MarkProgress, MarkUuid, MarkedFile,
        MovieDetails, MovieFile, MovieSummary, MovieUuid, PathId, PlexView, Rank,
        RankedClaimPredicate, SubjectAuditLog, TvEpisode, TvSeason, TvSeriesDetails, TvSeriesFile,
        TvSeriesSummary, TvSeriesUuid, View, ViewConfig, ViewMaterialization, ViewSyncStats,
        ViewUuid, WorkKind, disc_folder, is_disc_image,
    };
}
