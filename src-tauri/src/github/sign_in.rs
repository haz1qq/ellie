use reqwest::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use super::GitHubError;

pub(crate) const CALLBACK_PATH: &str = "/callback";
const MAX_CALLBACK_REQUEST_BYTES: usize = 8 * 1024;
const SUCCESS_PAGE: &str =
    "<!doctype html><html><body>GitHub sign-in complete. You can close this window.</body></html>";
const FAILURE_PAGE: &str = "<!doctype html><html><body>GitHub sign-in could not be completed. Return to Ellie and try again.</body></html>";

pub(crate) struct CallbackParameters {
    pub code: String,
    pub state: String,
}

pub(crate) async fn read_callback(
    stream: &mut TcpStream,
) -> Result<CallbackParameters, GitHubError> {
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    loop {
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() == MAX_CALLBACK_REQUEST_BYTES {
            return Err(GitHubError::invalid_input());
        }
        let remaining = MAX_CALLBACK_REQUEST_BYTES - request.len();
        let read_length = chunk.len().min(remaining);
        let read = stream
            .read(&mut chunk[..read_length])
            .await
            .map_err(|_| GitHubError::network_unavailable())?;
        if read == 0 {
            return Err(GitHubError::invalid_input());
        }
        request.extend_from_slice(&chunk[..read]);
    }

    parse_request(&request)
}

pub(crate) async fn respond(stream: &mut TcpStream, success: bool) {
    let (status, body) = if success {
        ("200 OK", SUCCESS_PAGE)
    } else {
        ("400 Bad Request", FAILURE_PAGE)
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn parse_request(request: &[u8]) -> Result<CallbackParameters, GitHubError> {
    let request = std::str::from_utf8(request).map_err(|_| GitHubError::invalid_input())?;
    let request_line = request
        .split("\r\n")
        .next()
        .ok_or_else(GitHubError::invalid_input)?;
    let mut parts = request_line.split_ascii_whitespace();
    let method = parts.next().ok_or_else(GitHubError::invalid_input)?;
    let target = parts.next().ok_or_else(GitHubError::invalid_input)?;
    let version = parts.next().ok_or_else(GitHubError::invalid_input)?;
    if parts.next().is_some()
        || method != "GET"
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || !target.starts_with('/')
        || target.starts_with("//")
        || target.len() > MAX_CALLBACK_REQUEST_BYTES
    {
        return Err(GitHubError::invalid_input());
    }

    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| GitHubError::invalid_input())?;
    if url.path() != CALLBACK_PATH || url.fragment().is_some() {
        return Err(GitHubError::invalid_input());
    }

    let mut code = None;
    let mut state = None;
    for (key, value) in url.query_pairs() {
        let slot = match key.as_ref() {
            "code" => &mut code,
            "state" => &mut state,
            _ => return Err(GitHubError::invalid_input()),
        };
        if slot.replace(value.into_owned()).is_some() {
            return Err(GitHubError::invalid_input());
        }
    }
    let code = code.filter(|value| !value.is_empty());
    let state = state.filter(|value| !value.is_empty());
    match (code, state) {
        (Some(code), Some(state)) => Ok(CallbackParameters { code, state }),
        _ => Err(GitHubError::invalid_input()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_parser_accepts_only_one_code_and_state_on_the_callback_path() {
        let parsed = parse_request(
            b"GET /callback?code=sanitized-code&state=sanitized-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        )
        .expect("valid callback");
        assert_eq!(parsed.code, "sanitized-code");
        assert_eq!(parsed.state, "sanitized-state");

        for request in [
            "POST /callback?code=a&state=b HTTP/1.1\r\n\r\n",
            "GET /other?code=a&state=b HTTP/1.1\r\n\r\n",
            "GET /callback?code=a&state=b&extra=c HTTP/1.1\r\n\r\n",
            "GET /callback?code=a&code=b&state=c HTTP/1.1\r\n\r\n",
            "GET /callback?code=a HTTP/1.1\r\n\r\n",
        ] {
            assert!(parse_request(request.as_bytes()).is_err(), "{request}");
        }
    }
}
