//! Errors: what the knowledge base couldn't answer, which interpretation
//! notes and goes on without.

use super::*;

/// Fails to search for a file whose title says it's broken, finds TMDB
/// movie 1 for the rest, and can't fetch any movie's details.
struct FlakyKb;

#[async_trait::async_trait]
impl KnowledgeBase for FlakyKb {
    async fn search_for_claims(
        &self,
        fields: &RankedSearchFields,
    ) -> Result<(Vec<KbResult>, Vec<KbQueryEvent>), KnowledgeBaseError> {
        if fields.titles.iter().any(|t| t.value.contains("broken")) {
            return Err(anyhow::anyhow!("the search timed out").into());
        }
        let claim = Ranked::new;
        Ok((
            vec![vec![
                claim(10.0, ClaimPredicate::MovieExternalId(ExternalId::Tmdb(1))),
                claim(10.0, ClaimPredicate::MovieTitle("Exist".into())),
            ]],
            vec![],
        ))
    }

    async fn get_movie_details(&self, _id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError> {
        Err(anyhow::anyhow!("503 Service Unavailable").into())
    }

    async fn get_tv_series_details(
        &self,
        _id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError> {
        Ok(None)
    }
}

/// Searches the knowledge base for the files' claims, and what that noted.
async fn search(
    files: &[(PathId, FileFacts)],
) -> (
    Result<(), KnowledgeBaseError>,
    Vec<NewInterpretationError>,
    HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
) {
    let mut logger = AuditLogger::default();
    let mut claims = run_heuristics(files);
    deactivate_duplicate_claims(&mut claims);
    let mut errors = Vec::new();
    let result =
        add_claims_from_knowledge_base(&FlakyKb, files, &mut logger, &mut claims, &mut errors)
            .await;
    (result, errors, claims)
}

#[tokio::test]
async fn a_failed_search_is_noted_and_the_rest_go_on() {
    let files = [
        video(1, "Broken.2013.mkv", 90.0),
        video(2, "Exist.2013.mkv", 90.0),
    ];
    let (result, errors, claims) = search(&files).await;
    assert!(result.is_ok());

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file_id, Some(FileMeasurementRowId(1)));
    assert!(errors[0].subject.contains("Broken.2013.mkv"));
    assert!(errors[0].error.contains("timed out"));

    // The other file still got the knowledge base's answer.
    let kb_claims = |subject: usize| {
        claims[&subject]
            .iter()
            .filter(|c| matches!(c.source, ClaimSource::KnowledgeBase { .. }))
            .count()
    };
    assert_eq!(kb_claims(0), 0);
    assert_eq!(kb_claims(1), 2);
}

#[tokio::test]
async fn every_search_failing_fails_the_interpretation() {
    let files = [
        video(1, "Broken.2013.mkv", 90.0),
        video(2, "Also.Broken.2014.mkv", 90.0),
    ];
    let (result, errors, _) = search(&files).await;
    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("every knowledge base search failed (2)"));
    assert!(error.contains("timed out"));
    assert_eq!(errors.len(), 2);
}

#[tokio::test]
async fn a_failed_details_lookup_leaves_the_work_in_the_catalog() {
    let files = [video(1, "Exist.2013.mkv", 90.0)];
    let mut logger = AuditLogger::default();
    let mut claims = run_heuristics(&files);
    deactivate_duplicate_claims(&mut claims);
    let mut errors = Vec::new();
    add_claims_from_knowledge_base(&FlakyKb, &files, &mut logger, &mut claims, &mut errors)
        .await
        .unwrap();
    apply_belief_policy(&mut claims);
    let audit_logs = logger.finish(claims.clone());
    let mut works = build_works(&files, claims);
    enrich_with_kb_data(&FlakyKb, &mut works, &mut errors).await;
    let catalog = new_catalog(&files, works, audit_logs);

    assert_eq!(
        movies_by_id(&catalog),
        vec![(Some(1), vec![(1, Some(FileRole::Main))])]
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file_id, None);
    assert!(
        errors[0].subject.contains("TMDB 1"),
        "{}",
        errors[0].subject
    );
    assert!(errors[0].error.contains("503"));
}
