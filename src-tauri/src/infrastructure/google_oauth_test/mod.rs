use std::collections::HashMap;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use super::google_oauth::{DesktopOAuthError, DesktopOAuthSession, MAX_IN_FLIGHT_CALLBACKS};

const CLIENT_ID: &str = "gdom-test.apps.googleusercontent.com";

fn query(url: &str) -> HashMap<String, String> {
    reqwest::Url::parse(url)
        .expect("authorization URL is valid")
        .query_pairs()
        .into_owned()
        .collect()
}

async fn send_callback(redirect_uri: &str, query: &str) -> String {
    send_callback_target(redirect_uri, &format!("/?{query}")).await
}

async fn send_callback_target(redirect_uri: &str, target: &str) -> String {
    let redirect = reqwest::Url::parse(redirect_uri).expect("redirect URI is valid");
    let address = format!(
        "{}:{}",
        redirect.host_str().expect("redirect URI has a host"),
        redirect.port().expect("redirect URI has an ephemeral port")
    );
    let mut stream = TcpStream::connect(address)
        .await
        .expect("callback client connects");
    stream
        .write_all(format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
        .await
        .expect("callback request writes");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("callback response reads");
    response
}

mod authorization;
mod lifecycle;
mod validation;
