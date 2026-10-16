//! A cache for TMDB's responses that keeps none for longer than TMDB's API
//! terms allow.

use http_cache_reqwest::{CACacheManager, CacheManager, HttpResponse};
use http_cache_semantics::CachePolicy;
use std::time::{Duration, SystemTime};

/// TMDB's API terms forbid caching anything from TMDB for longer than six
/// months.
const MAX_CACHE_AGE: Duration = Duration::from_secs(180 * 24 * 60 * 60);

/// What the cache's methods fail with.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The cache, except that a response cached for [`MAX_CACHE_AGE`] is deleted
/// when it's next asked for, so that it's fetched again. The cache otherwise
/// serves responses however old they are.
pub(crate) struct ExpiringCache(pub(crate) CACacheManager);

#[async_trait::async_trait]
impl CacheManager for ExpiringCache {
    async fn get(&self, cache_key: &str) -> Result<Option<(HttpResponse, CachePolicy)>, BoxError> {
        match self.0.get(cache_key).await? {
            Some((_, policy)) if policy.age(SystemTime::now()) >= MAX_CACHE_AGE => {
                // Fails if another response had the same contents and they
                // were deleted with it, which leaves nothing to serve anyway.
                let _ = self.0.delete(cache_key).await;
                Ok(None)
            }
            cached => Ok(cached),
        }
    }

    async fn put(
        &self,
        cache_key: String,
        res: HttpResponse,
        policy: CachePolicy,
    ) -> Result<HttpResponse, BoxError> {
        self.0.put(cache_key, res, policy).await
    }

    async fn delete(&self, cache_key: &str) -> Result<(), BoxError> {
        self.0.delete(cache_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_cache::{HttpHeaders, HttpVersion};

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// Caches a response with `body` under `key`, as if received `age` ago.
    async fn put(cache: &ExpiringCache, key: &str, body: &[u8], age: Duration) {
        let url = format!("https://api.themoviedb.org/3/{key}");
        let response = HttpResponse {
            body: body.to_vec(),
            headers: HttpHeaders::new(),
            status: 200,
            url: url.parse().unwrap(),
            version: HttpVersion::Http11,
        };
        let policy = CachePolicy::new_options(
            &http::Request::get(url).body(()).unwrap(),
            &http::Response::new(()),
            SystemTime::now() - age,
            Default::default(),
        );
        cache.put(key.into(), response, policy).await.unwrap();
    }

    #[tokio::test]
    async fn a_response_cached_too_long_is_deleted_when_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ExpiringCache(CACacheManager::new(dir.path().into(), true));
        put(&cache, "old", b"old", 181 * DAY).await;
        put(&cache, "new", b"new", DAY).await;

        assert!(cache.get("old").await.unwrap().is_none());
        assert_eq!(cache.get("new").await.unwrap().unwrap().0.body, b"new");
        // Its contents are gone, not just unlisted.
        let contents = walkdir::WalkDir::new(dir.path().join("content-v2"))
            .into_iter()
            .filter(|e| e.as_ref().unwrap().file_type().is_file())
            .count();
        assert_eq!(contents, 1);
    }
}
