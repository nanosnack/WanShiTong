pub use wanshitong_app::{AppContext, CoreContext};

use topcoat::context::{Cx, app_context};
use wanshitong_app::{self as app, AppError};

#[derive(Clone)]
pub struct WebContext {
    pub(crate) app: AppContext,
}

impl WebContext {
    pub fn new(app: AppContext) -> Self {
        Self { app }
    }

    pub async fn shutdown(&self) -> Result<(), AppError> {
        app::commands::shutdown(&self.app).await
    }
}

/// The application context registered with the topcoat router.
pub(crate) fn app(cx: &Cx) -> &AppContext {
    &app_context::<WebContext>(cx).app
}
