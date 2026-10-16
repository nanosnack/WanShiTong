use reqwest::StatusCode;

pub trait ResponseExt: Sized {
    /// If the response has an HTTP error status code, then consume the response
    /// body as text and return it as an [`ErrorResponse`].
    async fn check_error(self) -> Result<Self, ErrorResponse>;
}

#[derive(Debug, thiserror::Error)]
pub struct ErrorResponse {
    pub status: StatusCode,
    pub body: Option<String>,
    pub source: reqwest::Error,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Self {
            status,
            body,
            source,
        } = self;
        write!(f, "Got HTTP error response\nStatusCode={status}")?;
        if let Some(body) = body {
            write!(f, "\nResponse Body:\n{body}")?;
        }
        write!(f, "\nSource: {source}")?;
        Ok(())
    }
}

impl ResponseExt for reqwest::Response {
    async fn check_error(self) -> Result<Self, ErrorResponse> {
        if let Err(e) = self.error_for_status_ref() {
            let body = self.text().await.ok();
            Err(ErrorResponse {
                status: e.status().expect("reqwest said it was a status error"),
                body,
                source: e,
            })
        } else {
            Ok(self)
        }
    }
}
