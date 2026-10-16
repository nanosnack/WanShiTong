use topcoat::{
    Error,
    router::{
        StatusCode,
        error::{
            BadRequestError, ContentTooLargeError, NotFoundError, RedirectError, RewriteError,
            SeeOther,
        },
    },
};
use wanshitong_app::{AppError, AppErrorContext, CoreErrorContext, report_error};

/// Why a page failed, as shown in its place by the layout.
///
/// Handlers return `topcoat::Result`, whose error type is type-erased, so this
/// is built from it in one place: [`PageError::classify`]. A wrong HTTP method
/// never gets this far; the router answers it before any layout runs.
#[derive(Debug)]
pub enum PageError {
    /// No such page or record.
    NotFound,
    /// A malformed request: path parameter, query string or form body.
    BadRequest(String),
    /// A request body over the router's size limit.
    ContentTooLarge,
    /// The application refused the request, such as a duplicate name.
    Rejected(StatusCode, String),
    /// Anything that isn't the client's fault. Its details go to the server
    /// log, not the page.
    Internal(Error),
}

impl PageError {
    /// Classifies the error a page failed with.
    ///
    /// Errors that are control flow rather than failures (redirects and
    /// rewrites) are handed back, for the router to act on.
    pub fn classify(error: Error) -> Result<Self, Error> {
        if error.is::<SeeOther>() || error.is::<RedirectError>() || error.is::<RewriteError>() {
            return Err(error);
        }

        let page_error = if error.is::<NotFoundError>() {
            Self::NotFound
        } else if let Some(bad_request) = error.downcast_ref::<BadRequestError>() {
            Self::BadRequest(bad_request.to_string())
        } else if error.is::<ContentTooLargeError>() {
            Self::ContentTooLarge
        } else if let Some(app_error) = error.downcast_ref::<AppError>() {
            match app_error_status(app_error) {
                StatusCode::NOT_FOUND => Self::NotFound,
                status if status.is_client_error() => {
                    Self::Rejected(status, app_error_message(app_error))
                }
                _ => Self::Internal(error),
            }
        } else {
            Self::Internal(error)
        };

        if let Self::Internal(error) = &page_error {
            // An application error's causes are in its report, not in a
            // source chain, so it's logged as the report.
            match error.downcast_ref::<AppError>() {
                Some(app_error) => report_error!(app_error, "Request failed"),
                None => tracing::error!("{error:?}"),
            }
        }
        Ok(page_error)
    }

    /// Logs `error` if it isn't the client's fault, and hands it back.
    ///
    /// For route handlers: unlike pages, their errors never reach the layout's
    /// [`classify`](Self::classify), and topcoat answers them without logging.
    pub fn log(error: Error) -> Error {
        let _ = Self::classify(error.clone());
        error
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::ContentTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Rejected(status, _) => *status,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// What the page says about the failure.
    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "This page doesn't exist.".to_string(),
            Self::BadRequest(message) | Self::Rejected(_, message) => message.clone(),
            Self::ContentTooLarge => "The submitted form was too large.".to_string(),
            Self::Internal(_) => {
                "Something went wrong. The details are in the server log.".to_string()
            }
        }
    }
}

/// What a page or API response says about an application error: the core's
/// own message when the failure came from there, since the application adds
/// no words of its own above it.
pub(crate) fn app_error_message(error: &AppError) -> String {
    match error.core_context() {
        Some(core_error) => core_error.to_string(),
        None => error.to_string(),
    }
}

/// The HTTP status an application error is reported with.
pub(crate) fn app_error_status(error: &AppError) -> StatusCode {
    if let Some(core_error) = error.core_context() {
        return core_error_status(core_error);
    }
    match error.context() {
        AppErrorContext::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
        AppErrorContext::Watch(_) => StatusCode::INTERNAL_SERVER_ERROR,
        AppErrorContext::Core => StatusCode::INTERNAL_SERVER_ERROR,
        AppErrorContext::DataSourcesNotFound(_) => StatusCode::CONFLICT,
        AppErrorContext::NothingToImport => StatusCode::BAD_REQUEST,
        AppErrorContext::IngestNotFound(_) => StatusCode::NOT_FOUND,
        AppErrorContext::InterpretationNotFound(_) => StatusCode::NOT_FOUND,
        AppErrorContext::InterpretationNotSucceeded(_) => StatusCode::CONFLICT,
        AppErrorContext::KnowledgeBase(_) => StatusCode::BAD_GATEWAY,
        AppErrorContext::NoCatalog => StatusCode::CONFLICT,
        AppErrorContext::NoKnowledgeBase => StatusCode::SERVICE_UNAVAILABLE,
        AppErrorContext::ProbeFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
        AppErrorContext::ViewNotFound(_) => StatusCode::NOT_FOUND,
        AppErrorContext::ViewSync(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// The HTTP status a core failure is reported with.
fn core_error_status(error: &CoreErrorContext) -> StatusCode {
    match error {
        CoreErrorContext::AssertionNotFound(_) => StatusCode::NOT_FOUND,
        CoreErrorContext::AssertionNotRetractable { .. } => StatusCode::CONFLICT,
        CoreErrorContext::AssertionWithoutFiles => StatusCode::BAD_REQUEST,
        CoreErrorContext::ChangesetNotFound(_) => StatusCode::NOT_FOUND,
        CoreErrorContext::ChangesetNotOpen(_) => StatusCode::CONFLICT,
        CoreErrorContext::InvalidConfidence(_) => StatusCode::BAD_REQUEST,
        CoreErrorContext::InvalidChangesetAuthor(_) => StatusCode::BAD_REQUEST,
        CoreErrorContext::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::DataSourceAndIngestNotFound(..) => StatusCode::NOT_FOUND,
        CoreErrorContext::DataSourceDeleted(_) => StatusCode::CONFLICT,
        CoreErrorContext::IngestNotSucceeded(_) => StatusCode::CONFLICT,
        CoreErrorContext::DataSourceNotFound(_) => StatusCode::NOT_FOUND,
        CoreErrorContext::DataSourceNameExists(_) => StatusCode::CONFLICT,
        CoreErrorContext::DataSourceOverlapsView { .. } => StatusCode::CONFLICT,
        CoreErrorContext::DataSourceProbe(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::Decode(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::InvalidCatalog(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::InvalidConfig(..) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::InvalidDataSourceName(_) => StatusCode::BAD_REQUEST,
        CoreErrorContext::InvalidInterpretationName(_) => StatusCode::BAD_REQUEST,
        CoreErrorContext::InvalidViewName(_) => StatusCode::BAD_REQUEST,
        CoreErrorContext::InvalidViewRootPath(_) => StatusCode::BAD_REQUEST,
        CoreErrorContext::InterpretationNotFound(_) => StatusCode::NOT_FOUND,
        CoreErrorContext::MarkWithoutFiles => StatusCode::BAD_REQUEST,
        CoreErrorContext::PartOfDisc { .. } => StatusCode::BAD_REQUEST,
        CoreErrorContext::InterpretationOfUnacceptedChangeset { .. } => StatusCode::CONFLICT,
        CoreErrorContext::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::UnknownFileRole(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::UnknownViewKind(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::UnknownChangesetState(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::UnknownDiscKind(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreErrorContext::ViewNameExists(_) => StatusCode::CONFLICT,
        CoreErrorContext::ViewNotFound(_) => StatusCode::NOT_FOUND,
        CoreErrorContext::ViewOverlapsDataSource { .. } => StatusCode::CONFLICT,
        CoreErrorContext::ViewOverlapsView { .. } => StatusCode::CONFLICT,
    }
}
