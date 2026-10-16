use crate::{
    CoreContext, CoreError, CoreErrorContext,
    changesets::{find_changeset, select_interpretation_assertions},
    current_build,
    domain::{ChangesetState, ChangesetUuid, IngestUuid, InterpretationUuid},
    queries::list_latest_successful_ingests,
};
use std::collections::{BTreeSet, HashSet};

/// The newest of a changeset's interpretations that hasn't failed and is
/// still up to date: of the latest ingests, and of the assertions a new one
/// would include, made by this build of WST, as another, e.g. before a
/// deploy, would make another catalog of them. For an open changeset, that's what accepting it would make;
/// for an accepted one, what it made, if nothing has changed since. None if
/// there's no such interpretation, and one is needed.
pub async fn find_current_changeset_interpretation(
    ctx: &CoreContext,
    uuid: ChangesetUuid,
) -> Result<Option<InterpretationUuid>, CoreError> {
    let latest_ingests: HashSet<IngestUuid> = list_latest_successful_ingests(ctx)
        .await?
        .into_iter()
        .map(|(_, ingest)| ingest)
        .collect();
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;
    let changeset = find_changeset(&mut txn, uuid).await?;
    let Some(build) = current_build() else {
        return Ok(None);
    };
    let pending = (changeset.state == ChangesetState::Open).then_some(changeset.id);
    let expected: BTreeSet<i64> = select_interpretation_assertions(&mut txn, pending)
        .await?
        .into_iter()
        .collect();

    let candidates = sqlx::query!(
        r#"
        SELECT id AS "id!", uuid AS "uuid!: InterpretationUuid"
        FROM interpretations
        WHERE changeset_id = ? AND error IS NULL AND build = ?
        ORDER BY started_at DESC, id DESC
        "#,
        changeset.id,
        build,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    for candidate in candidates {
        let assertions: BTreeSet<i64> = sqlx::query_scalar!(
            r#"
            SELECT assertion_id AS "id!"
            FROM interpretation_assertions
            WHERE interpretation_id = ?
            "#,
            candidate.id,
        )
        .fetch_all(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?
        .into_iter()
        .collect();
        let ingests: HashSet<IngestUuid> = sqlx::query_scalar!(
            r#"
            SELECT i.uuid AS "uuid!: IngestUuid"
            FROM interpretation_ingests ii
            JOIN ingests i
                ON i.id = ii.ingest_id
            WHERE ii.interpretation_id = ?
            "#,
            candidate.id,
        )
        .fetch_all(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?
        .into_iter()
        .collect();
        if assertions == expected && ingests == latest_ingests {
            return Ok(Some(candidate.uuid));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{NewChangeset, create_changeset, create_changeset_interpretation};
    use crate::domain::InterpretationRowId;

    async fn made_by(ctx: &CoreContext, interpretation: InterpretationRowId, build: Option<&str>) {
        sqlx::query!(
            "UPDATE interpretations SET build = ? WHERE id = ?",
            build,
            interpretation
        )
        .execute(&ctx.writer)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn another_builds_interpretation_is_not_current() {
        let ctx = CoreContext::new_in_memory().await.unwrap();
        let changeset = create_changeset(
            &ctx,
            NewChangeset {
                author: "test".into(),
                description: "nothing".into(),
            },
        )
        .await
        .unwrap();
        let preview = create_changeset_interpretation(&ctx, "preview", &[], changeset)
            .await
            .unwrap();
        let current = || find_current_changeset_interpretation(&ctx, changeset);
        assert_eq!(current().await.unwrap(), Some(preview.uuid));

        // E.g. the build before a deploy.
        made_by(&ctx, preview.row_id, Some("another build")).await;
        assert_eq!(current().await.unwrap(), None);
        // Made before builds were recorded.
        made_by(&ctx, preview.row_id, None).await;
        assert_eq!(current().await.unwrap(), None);
    }
}
