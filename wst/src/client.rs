//! The CLI's connection to a running WST service: plain HTTP, which is what
//! WST serves, so there's no TLS to set up and no certificates to find.

use hyper_util::client::legacy::{Client as HttpClient, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use wanshitong_client::RequestBody;

pub type Client = wanshitong_client::Client<HttpClient<HttpConnector, RequestBody>>;

/// A client of the service at `url`.
pub fn client(url: &str) -> Client {
    Client::new(HttpClient::builder(TokioExecutor::new()).build_http(), url)
}
