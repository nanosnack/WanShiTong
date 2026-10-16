//! A client of WST's JSON API, `/api/v1`, with a
//! method for each endpoint, in the types of `wanshitong_api::v1`.
//!
//! It sends its requests through any [`Service`] that takes an HTTP request
//! and answers it. The `wst` CLI's is a connection to a running service; the
//! API's tests hand it the service's router itself, so that they exercise the
//! client too, with no socket between them.

use bytes::Bytes;
use http::{Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full};
use serde::{Serialize, de::DeserializeOwned};
use std::future::poll_fn;
use tower_service::Service;
use uuid::Uuid;
use wanshitong_api::v1::{
    self, AcceptedChangeset, ActiveAssertions, CatalogDiff, Changeset, Changesets, ChangesetsQuery,
    CreatedAssertions, DiffQuery, ErrorResponse, FileEvidence, FileQuery, Health, ImportAssertions,
    ImportedAssertions, KbMovie, KbSearchQuery, KbSearchResults, KbTvSeries, Marks, MarksQuery,
    NewAssertions, NewChangeset, NewRetraction, UnidentifiedQuery, UnidentifiedVideos,
    WorkEvidence, WorkQuery,
};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The body of a request the client sends, whole: what its [`Service`] takes.
pub type RequestBody = Full<Bytes>;

/// Why a request failed: the service couldn't be reached, or answered with
/// an error, or with something other than the API's answer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("couldn't reach WST at {url}")]
    Unreachable {
        url: String,
        #[source]
        source: BoxError,
    },
    #[error("couldn't read the response from {url}")]
    Body {
        url: String,
        #[source]
        source: BoxError,
    },
    /// The API refused the request, and said why.
    #[error("{}", error.message)]
    Api {
        status: StatusCode,
        error: v1::Error,
    },
    /// An error response that isn't the API's, e.g. a proxy's.
    #[error("{url} responded {status}: {body}")]
    NotTheApi {
        url: String,
        status: StatusCode,
        body: String,
    },
    /// A response that isn't what the endpoint answers.
    #[error("unexpected response from {url}: {body}")]
    Unexpected {
        url: String,
        body: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("couldn't make a request for {url}")]
    Request {
        url: String,
        #[source]
        source: BoxError,
    },
}

/// A client of the WST service at a URL, e.g. `http://localhost:6765`,
/// reached through `service`.
#[derive(Clone, Debug)]
pub struct Client<S> {
    service: S,
    url: String,
}

impl<S> Client<S> {
    pub fn new(service: S, url: &str) -> Self {
        Self {
            service,
            url: url.trim_end_matches('/').to_owned(),
        }
    }
}

impl<S, B> Client<S>
where
    S: Service<Request<RequestBody>, Response = Response<B>> + Clone,
    S::Error: Into<BoxError>,
    B: http_body::Body<Data = Bytes>,
    B::Error: Into<BoxError>,
{
    /// `GET /api/v1/health`
    pub async fn health(&self) -> Result<Health, Error> {
        self.get("/health", &()).await
    }

    /// `GET /api/v1/unidentified`
    pub async fn get_unidentified_videos(
        &self,
        query: &UnidentifiedQuery,
    ) -> Result<UnidentifiedVideos, Error> {
        self.get("/unidentified", query).await
    }

    /// `GET /api/v1/files/evidence`
    pub async fn get_file_evidence(&self, query: &FileQuery) -> Result<FileEvidence, Error> {
        self.get("/files/evidence", query).await
    }

    /// `GET /api/v1/works/{uuid}`
    pub async fn get_work_evidence(
        &self,
        uuid: Uuid,
        query: &WorkQuery,
    ) -> Result<WorkEvidence, Error> {
        self.get(&format!("/works/{uuid}"), query).await
    }

    /// `GET /api/v1/kb/search`
    pub async fn search_knowledge_base(
        &self,
        query: &KbSearchQuery,
    ) -> Result<KbSearchResults, Error> {
        self.get("/kb/search", query).await
    }

    /// `GET /api/v1/kb/movies/{tmdb_id}`
    pub async fn get_kb_movie(&self, tmdb_id: u32) -> Result<KbMovie, Error> {
        self.get(&format!("/kb/movies/{tmdb_id}"), &()).await
    }

    /// `GET /api/v1/kb/tv-series/{tmdb_id}`
    pub async fn get_kb_tv_series(&self, tmdb_id: u32) -> Result<KbTvSeries, Error> {
        self.get(&format!("/kb/tv-series/{tmdb_id}"), &()).await
    }

    /// `POST /api/v1/changesets`
    pub async fn create_changeset(&self, changeset: &NewChangeset) -> Result<Changeset, Error> {
        self.post("/changesets", changeset).await
    }

    /// `GET /api/v1/changesets`
    pub async fn list_changesets(&self, query: &ChangesetsQuery) -> Result<Changesets, Error> {
        self.get("/changesets", query).await
    }

    /// `GET /api/v1/changesets/{uuid}`
    pub async fn get_changeset(&self, uuid: Uuid) -> Result<Changeset, Error> {
        self.get(&format!("/changesets/{uuid}"), &()).await
    }

    /// `POST /api/v1/changesets/{uuid}/assertions`
    pub async fn add_assertions(
        &self,
        changeset: Uuid,
        assertions: &NewAssertions,
    ) -> Result<CreatedAssertions, Error> {
        self.post(&format!("/changesets/{changeset}/assertions"), assertions)
            .await
    }

    /// `POST /api/v1/changesets/{uuid}/retractions`
    pub async fn retract(
        &self,
        changeset: Uuid,
        retraction: &NewRetraction,
    ) -> Result<Changeset, Error> {
        self.post(&format!("/changesets/{changeset}/retractions"), retraction)
            .await
    }

    /// `POST /api/v1/changesets/{uuid}/accept`
    pub async fn accept_changeset(&self, changeset: Uuid) -> Result<AcceptedChangeset, Error> {
        self.post(&format!("/changesets/{changeset}/accept"), &())
            .await
    }

    /// `POST /api/v1/changesets/{uuid}/discard`
    pub async fn discard_changeset(&self, changeset: Uuid) -> Result<Changeset, Error> {
        self.post(&format!("/changesets/{changeset}/discard"), &())
            .await
    }

    /// `POST /api/v1/changesets/{uuid}/diff`
    pub async fn diff_changeset(&self, changeset: Uuid) -> Result<CatalogDiff, Error> {
        self.post(&format!("/changesets/{changeset}/diff"), &())
            .await
    }

    /// `GET /api/v1/diff`
    pub async fn diff(&self, query: &DiffQuery) -> Result<CatalogDiff, Error> {
        self.get("/diff", query).await
    }

    /// `GET /api/v1/assertions`
    pub async fn list_assertions(&self) -> Result<ActiveAssertions, Error> {
        self.get("/assertions", &()).await
    }

    /// `POST /api/v1/assertions/import`
    pub async fn import_assertions(
        &self,
        import: &ImportAssertions,
    ) -> Result<ImportedAssertions, Error> {
        self.post("/assertions/import", import).await
    }

    /// `GET /api/v1/marks`
    pub async fn list_marks(&self, query: &MarksQuery) -> Result<Marks, Error> {
        self.get("/marks", query).await
    }

    /// `GET /api/v1{path}`, with `query` as its query string, as JSON. The
    /// methods above are this for each endpoint; it's for a request they
    /// don't make, e.g. a test's malformed one.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &impl Serialize,
    ) -> Result<T, Error> {
        let mut url = format!("{}/api/v1{path}", self.url);
        let query = serde_urlencoded::to_string(query).map_err(|e| Error::Request {
            url: url.clone(),
            source: e.into(),
        })?;
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }
        let request = Request::get(&url)
            .header(header::ACCEPT, "application/json")
            .body(Full::default())
            .map_err(|e| Error::Request {
                url: url.clone(),
                source: e.into(),
            })?;
        self.send(request).await
    }

    /// `POST /api/v1{path}`, with `body` as JSON, as JSON. As for [`get`](Self::get).
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, Error> {
        let url = format!("{}/api/v1{path}", self.url);
        let body = serde_json::to_vec(body).map_err(|e| Error::Request {
            url: url.clone(),
            source: e.into(),
        })?;
        let request = Request::post(&url)
            .header(header::ACCEPT, "application/json")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::from(body))
            .map_err(|e| Error::Request {
                url: url.clone(),
                source: e.into(),
            })?;
        self.send(request).await
    }

    async fn send<T: DeserializeOwned>(&self, request: Request<RequestBody>) -> Result<T, Error> {
        let url = request.uri().to_string();
        let unreachable = |source: S::Error| Error::Unreachable {
            url: self.url.clone(),
            source: source.into(),
        };
        let mut service = self.service.clone();
        poll_fn(|cx| service.poll_ready(cx))
            .await
            .map_err(unreachable)?;
        let (parts, body) = service
            .call(request)
            .await
            .map_err(unreachable)?
            .into_parts();
        let body = body
            .collect()
            .await
            .map_err(|e| Error::Body {
                url: url.clone(),
                source: e.into(),
            })?
            .to_bytes();
        if !parts.status.is_success() {
            return Err(match serde_json::from_slice::<ErrorResponse>(&body) {
                Ok(ErrorResponse { error }) => Error::Api {
                    status: parts.status,
                    error,
                },
                Err(_) => Error::NotTheApi {
                    url,
                    status: parts.status,
                    body: String::from_utf8_lossy(&body).into_owned(),
                },
            });
        }
        serde_json::from_slice(&body).map_err(|source| Error::Unexpected {
            url,
            body: String::from_utf8_lossy(&body).into_owned(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body::Body as _;
    use serde::Deserialize;
    use std::convert::Infallible;
    use std::future::{Ready, ready};
    use std::task::{Context, Poll};
    use wanshitong_api::v1::ErrorCode;

    /// A service that answers every request with `answer`'s response.
    #[derive(Clone)]
    struct Stub<F>(F);

    impl<F> Service<Request<RequestBody>> for Stub<F>
    where
        F: Fn(Request<RequestBody>) -> Response<Full<Bytes>>,
    {
        type Response = Response<Full<Bytes>>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Infallible>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request<RequestBody>) -> Self::Future {
            ready(Ok((self.0)(request)))
        }
    }

    fn answer(status: u16, body: &str) -> Response<Full<Bytes>> {
        Response::builder()
            .status(status)
            .body(Full::from(body.to_owned()))
            .unwrap()
    }

    #[derive(Deserialize)]
    struct Echo {
        method: String,
        uri: String,
        body: String,
    }

    type Answer = fn(Request<RequestBody>) -> Response<Full<Bytes>>;

    /// A service that says what it was asked.
    fn echo() -> Client<Stub<Answer>> {
        fn echo(request: Request<RequestBody>) -> Response<Full<Bytes>> {
            let (parts, body) = request.into_parts();
            let body = bytes_of(body);
            let echo = serde_json::json!({
                "method": parts.method.as_str(),
                "uri": parts.uri.to_string(),
                "body": String::from_utf8_lossy(&body),
            });
            answer(200, &echo.to_string())
        }
        Client::new(Stub(echo as Answer), "http://wst/")
    }

    /// A `Full` body's bytes, which it holds whole.
    fn bytes_of(body: RequestBody) -> Bytes {
        let mut body = body;
        let frame = std::pin::Pin::new(&mut body);
        match frame.poll_frame(&mut Context::from_waker(std::task::Waker::noop())) {
            Poll::Ready(Some(Ok(frame))) => frame.into_data().unwrap_or_default(),
            _ => Bytes::new(),
        }
    }

    #[tokio::test]
    async fn requests_name_the_endpoint_under_the_url_with_their_query() {
        let client = echo();
        let marks: Echo = client
            .get("/marks", &MarksQuery { done: true })
            .await
            .unwrap();
        assert_eq!(
            (marks.method.as_str(), marks.uri.as_str()),
            ("GET", "http://wst/api/v1/marks?done=true")
        );
        // Nothing to ask leaves the URL alone.
        let health: Echo = client.get("/health", &()).await.unwrap();
        assert_eq!(health.uri, "http://wst/api/v1/health");

        let changeset: Echo = client
            .post(
                "/changesets",
                &NewChangeset {
                    author: "agent".into(),
                    description: String::new(),
                },
            )
            .await
            .unwrap();
        assert_eq!(changeset.method, "POST");
        assert_eq!(changeset.uri, "http://wst/api/v1/changesets");
        assert_eq!(changeset.body, r#"{"author":"agent","description":""}"#);
    }

    #[tokio::test]
    async fn errors_say_whose_they_are() {
        let api = Client::new(
            Stub(|_| {
                answer(
                    409,
                    r#"{"error": {"code": "conflict", "message": "it's taken"}}"#,
                )
            }),
            "http://wst",
        );
        let error = api.health().await.unwrap_err();
        assert_eq!(error.to_string(), "it's taken");
        let Error::Api { status, error } = error else {
            panic!("{error:?}");
        };
        assert_eq!((status.as_u16(), error.code), (409, ErrorCode::Conflict));
        // A code this client doesn't know is still an API error.
        let newer = Client::new(
            Stub(|_| answer(418, r#"{"error": {"code": "teapot", "message": "no"}}"#)),
            "http://wst",
        );
        let Error::Api { error, .. } = newer.health().await.unwrap_err() else {
            panic!()
        };
        assert_eq!(error.code, ErrorCode::Unknown);

        let proxy = Client::new(Stub(|_| answer(502, "Bad Gateway")), "http://wst");
        assert_eq!(
            proxy.health().await.unwrap_err().to_string(),
            "http://wst/api/v1/health responded 502 Bad Gateway: Bad Gateway"
        );

        let odd = Client::new(Stub(|_| answer(200, "<html>")), "http://wst");
        assert!(matches!(
            odd.health().await.unwrap_err(),
            Error::Unexpected { body, .. } if body == "<html>"
        ));
    }
}
