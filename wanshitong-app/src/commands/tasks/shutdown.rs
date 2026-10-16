use crate::{AppContext, AppError};

pub async fn shutdown(ctx: &AppContext) -> Result<(), AppError> {
    let tasks = ctx.tasks.lock().await;
    for task in tasks.values() {
        task.abort();
    }
    Ok(())
}
