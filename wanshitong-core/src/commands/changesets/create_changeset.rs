use crate::{
    CoreContext, CoreError, CoreErrorContext, domain::ChangesetUuid, time::unix_now_i64,
    validation::validate_name,
};
use uuid::Uuid;

pub struct NewChangeset {
    /// Who makes it, e.g. "user", or an agent run's name.
    pub author: String,
    /// What it's for.
    pub description: String,
}

/// Opens a changeset, to make assertions and retractions in.
pub async fn create_changeset(
    ctx: &CoreContext,
    changeset: NewChangeset,
) -> Result<ChangesetUuid, CoreError> {
    let NewChangeset {
        author,
        description,
    } = changeset;
    let author = author.trim();
    validate_name(author).map_err(CoreErrorContext::InvalidChangesetAuthor)?;
    let description = description.trim();
    let uuid = ChangesetUuid(Uuid::new_v4());
    let now = unix_now_i64();
    sqlx::query!(
        r#"
        INSERT INTO changesets (uuid, author, description, created_at, state)
        VALUES (?, ?, ?, ?, 'open')
        "#,
        uuid,
        author,
        description,
        now,
    )
    .execute(&ctx.writer)
    .await
    .map_err(CoreErrorContext::from)?;
    Ok(uuid)
}
