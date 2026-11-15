//! Spotify sign-in with the Authorization Code + PKCE flow.
//!
//! Three independent grants may exist because Spotify treats them differently:
//!
//! - The shared and optional personal **Web API grants** use separate
//!   registered application identities, protected entries, and request sessions.
//! - The **playback grant** uses Spotify's desktop client identity, the one
//!   librespot streams with. Its access token is exchanged once for a
//!   reusable credential kept in the platform credential store.
//!
//! The browser does the password entry; this process only ever sees the
//! one-time authorization code that Spotify sends back to a loopback
//! listener.

use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Spotify's own desktop client identity, the one librespot streams with.
pub const PLAYBACK_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
pub const PLAYBACK_REDIRECT_PORT: u16 = 8898;

/// Leaximy's own Web API application. Upstream's shared application stopped
/// accepting this sign-in address, so Leaximy signs in through its own. Its
/// dashboard must list http://127.0.0.1:8989/login as a redirect URI, and in
/// Spotify's Development Mode only the users listed there can sign in.
pub const DEFAULT_WEB_CLIENT_ID: &str = "9b587d771287445fa1cae6123d50b282";
pub const WEB_REDIRECT_PORT: u16 = 8989;

pub const REDIRECT_PATH: &str = "/login";

/// Playback: what librespot needs to stream and join Spotify Connect.
pub const PLAYBACK_SCOPES: &[&str] = &["streaming"];

/// Web API: what visible features use, plus `user-read-private` for the
/// plan (Free or Premium), which decides whether local playback is offered
/// at all; no email.
pub const WEB_SCOPES: &[&str] = &[
    "ugc-image-upload",
    "playlist-modify-private",
    "playlist-modify-public",
    "playlist-read-collaborative",
    "playlist-read-private",
    "user-follow-modify",
    "user-follow-read",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-read-private",
    "user-read-recently-played",
    "user-top-read",
];

const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Refresh this long before the access token expires.
const REFRESH_MARGIN: Duration = Duration::from_secs(90);

/// One OAuth application identity and what it is asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub client_id: String,
    pub redirect_port: u16,
    pub scopes: &'static [&'static str],
}

impl Grant {
    pub fn playback() -> Self {
        Self {
            client_id: PLAYBACK_CLIENT_ID.to_string(),
            redirect_port: PLAYBACK_REDIRECT_PORT,
            scopes: PLAYBACK_SCOPES,
        }
    }

    pub fn shared_web_api() -> Self {
        Self {
            client_id: DEFAULT_WEB_CLIENT_ID.to_string(),
            redirect_port: WEB_REDIRECT_PORT,
            scopes: WEB_SCOPES,
        }
    }

    pub fn personal_web_api(client_id: &str) -> Result<Self> {
        let client_id = client_id.trim();
        if client_id.is_empty() {
            bail!("a personal Spotify Client ID is required");
        }
        Ok(Self {
            client_id: client_id.to_string(),
            redirect_port: WEB_REDIRECT_PORT,
            scopes: WEB_SCOPES,
        })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}{REDIRECT_PATH}", self.redirect_port)
    }
}

/// A started sign-in: the URL to open and the secrets needed to finish it.
#[derive(Clone, Debug)]
pub struct Flow {
    pub verifier: String,
    pub state: String,
    pub url: String,
}

fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

pub fn begin(grant: Grant) -> Flow {
    let verifier = random_token(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token(18);
    let show_dialog = if grant.client_id == PLAYBACK_CLIENT_ID {
        "&show_dialog=true"
    } else {
        ""
    };
    let url = format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={challenge}&state={state}&scope={}{show_dialog}",
        grant.client_id,
        urlencoding::encode(&grant.redirect_uri()),
        urlencoding::encode(&grant.scopes.join(" "))
    );
    Flow {
        verifier,
        state,
        url,
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Listens for Spotify's redirect and returns the authorization code.
///
/// Ends early when `cancel` flips to true (the user gave up) or after ten
/// minutes.
pub async fn wait_for_code(
    port: u16,
    expected_state: &str,
    mut cancel: watch::Receiver<bool>,
) -> Result<String> {
    let address: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("unable to listen on {address} for the Spotify redirect"))?;
    let deadline = tokio::time::sleep(LOGIN_TIMEOUT);
    tokio::pin!(deadline);

    loop {
        let (mut stream, _) = tokio::select! {
            accepted = listener.accept() => accepted.context("redirect listener failed")?,
            _ = cancel.changed() => {
                if *cancel.borrow() { bail!("sign-in cancelled"); }
                continue;
            }
            _ = &mut deadline => bail!("sign-in timed out; try again"),
        };

        let mut reader = BufReader::new(&mut stream);
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).await.is_err() {
            continue;
        }
        let outcome = parse_request_line(&request_line, expected_state);
        let (status, body) = match &outcome {
            Ok(_) => ("200 OK", success_page()),
            Err(error) => ("400 Bad Request", failure_page(&error.to_string())),
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        match outcome {
            Ok(code) => return Ok(code),
            Err(error) => {
                // A favicon request or a stale tab is not the redirect; keep waiting.
                log::debug!("ignored request on the redirect listener: {error}");
                continue;
            }
        }
    }
}

fn parse_request_line(line: &str, expected_state: &str) -> Result<String> {
    let target = line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("malformed request"))?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != REDIRECT_PATH {
        bail!("unexpected redirect path");
    }
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = urlencoding::decode(value)
            .map(|value| value.into_owned())
            .unwrap_or_else(|_| value.to_string());
        match key {
            "code" => code = Some(value),
            "state" => state = Some(value),
            "error" => error = Some(value),
            _ => {}
        }
    }
    if error.is_some() {
        bail!("Spotify refused the sign-in. Try again.");
    }
    if state.as_deref() != Some(expected_state) {
        bail!("state mismatch");
    }
    code.ok_or_else(|| anyhow!("Spotify did not return an authorization code"))
}

pub async fn exchange_code(
    http: &reqwest::Client,
    grant: &Grant,
    code: &str,
    verifier: &str,
) -> Result<TokenResponse> {
    token_request(
        http,
        &[
            ("client_id", grant.client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", grant.redirect_uri().as_str()),
            ("code_verifier", verifier),
        ],
    )
    .await
    .map_err(|error| anyhow!("{error}"))
}

pub async fn refresh(
    http: &reqwest::Client,
    client_id: &str,
    refresh_token: &str,
) -> std::result::Result<TokenResponse, TokenEndpointError> {
    token_request(
        http,
        &[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ],
    )
    .await
}

#[derive(Debug, thiserror::Error)]
pub enum TokenEndpointError {
    /// Spotify refused the grant itself; only the browser can mint a new one.
    #[error("Spotify rejected the token request ({status}): {detail}")]
    Rejected { status: u16, detail: String },
    /// The endpoint could not answer; the grant may still be valid.
    #[error("token request failed: {0}")]
    Unreachable(String),
}

async fn token_request(
    http: &reqwest::Client,
    form: &[(&str, &str)],
) -> std::result::Result<TokenResponse, TokenEndpointError> {
    let response = http
        .post(TOKEN_URL)
        .form(form)
        .send()
        .await
        .map_err(|error| TokenEndpointError::Unreachable(error.to_string()))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    decode_token_response(status, &text)
}

fn decode_token_response(
    status: reqwest::StatusCode,
    text: &str,
) -> std::result::Result<TokenResponse, TokenEndpointError> {
    if status.is_client_error() {
        let value = serde_json::from_str::<serde_json::Value>(text).ok();
        // OAuth descriptions and malformed bodies may contain usable grants.
        // Report only known classifications, never authorization response text.
        let detail = match value.as_ref().and_then(|value| value["error"].as_str()) {
            Some("invalid_grant") => "The grant expired or was revoked. Sign in again.",
            Some("invalid_client") => "Check the Spotify application's Client ID.",
            Some("invalid_scope") => "Spotify did not accept the requested permissions.",
            Some("access_denied") => "Spotify authorization was declined.",
            _ => "Sign in again and check the Spotify application settings.",
        };
        return Err(TokenEndpointError::Rejected {
            status: status.as_u16(),
            detail: detail.into(),
        });
    }
    if !status.is_success() {
        return Err(TokenEndpointError::Unreachable(format!(
            "Spotify answered {status}"
        )));
    }
    serde_json::from_str(text).map_err(|_| {
        TokenEndpointError::Unreachable("Spotify returned an unreadable token response".into())
    })
}

/// The Web API grant, serialized only inside protected credential storage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoredToken {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at: u64,
    #[serde(default)]
    pub scope: String,
}

impl StoredToken {
    pub fn from_response(
        client_id: &str,
        response: TokenResponse,
        previous_refresh: Option<&str>,
    ) -> Result<Self> {
        let refresh_token = response
            .refresh_token
            .or_else(|| previous_refresh.map(str::to_string))
            .ok_or_else(|| anyhow!("Spotify did not return a refresh token"))?;
        Ok(Self {
            client_id: client_id.to_string(),
            access_token: response.access_token,
            refresh_token,
            expires_at: now_secs() + response.expires_in.unwrap_or(3600),
            scope: response.scope.unwrap_or_default(),
        })
    }

    pub fn needs_refresh(&self) -> bool {
        now_secs() + REFRESH_MARGIN.as_secs() >= self.expires_at
    }

    pub fn expired(&self) -> bool {
        now_secs() >= self.expires_at
    }

    /// Whether the grant covers every scope in `scopes`. A grant cannot be
    /// widened by a refresh, only by the browser.
    pub fn has_scopes(&self, scopes: &[&str]) -> bool {
        let granted: Vec<&str> = self.scope.split_whitespace().collect();
        scopes.iter().all(|scope| granted.contains(scope))
    }

    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    // Legacy file fixtures only. Production persistence lives in credentials.rs.
    #[cfg(test)]
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        let temporary = path.with_extension("json.tmp");
        write_private(&temporary, text.as_bytes())?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    }

    // Legacy file fixtures only. Production persistence lives in credentials.rs.
    #[cfg(test)]
    pub fn remove(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    // Legacy file fixtures only. Production persistence lives in credentials.rs.
    #[cfg(test)]
    pub fn migrate_legacy(legacy: &Path, shared: &Path, personal: &Path) -> Result<()> {
        let Some(token) = Self::load(legacy) else {
            return Ok(());
        };
        let target = if token.client_id == DEFAULT_WEB_CLIENT_ID {
            shared
        } else {
            personal
        };
        if let Some(existing) = Self::load(target) {
            if existing != token {
                return Ok(());
            }
        } else {
            token.save(target)?;
        }
        Self::remove(legacy);
        Ok(())
    }
}

#[cfg(test)]
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.flush()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn page(title: &str, heading: &str, body: &str, accent: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title>\
<style>:root{{color-scheme:dark}}body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a142e;color:#f6f8fc;font-family:Inter,system-ui,sans-serif}}\
main{{max-width:28rem;padding:2.5rem;border-radius:1.25rem;background:#0e1b3b;box-shadow:0 20px 60px rgba(0,0,0,.5);text-align:center}}\
.mark{{width:72px;height:72px;border-radius:18px;box-shadow:0 0 0 3px {accent};margin:0 auto 1.25rem}}\
.mark svg{{display:block;width:72px;height:72px}}h1{{font-size:1.4rem;margin:.25rem 0 .5rem}}p{{color:#b3bfd9;line-height:1.5;margin:0}}</style>\
<main><div class=\"mark\"><svg viewBox=\"16 16 480 480\" aria-hidden=\"true\"><defs><linearGradient id=\"f\" x1=\"0\" y1=\"16\" x2=\"0\" y2=\"496\" gradientUnits=\"userSpaceOnUse\"><stop offset=\"0\" stop-color=\"#2446a0\"/><stop offset=\"1\" stop-color=\"#0a163a\"/></linearGradient></defs><rect x=\"16\" y=\"16\" width=\"480\" height=\"480\" rx=\"116\" fill=\"url(#f)\"/><path d=\"M150,92 L150,322 L296,322\" fill=\"none\" stroke=\"#fff\" stroke-width=\"52\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/><path d=\"M268,164 L352,248 M352,164 L268,248\" fill=\"none\" stroke=\"#f5b335\" stroke-width=\"40\" stroke-linecap=\"round\"/><path d=\"M218.26,427.73 L217.16,426.83 L216.26,425.34 L215.03,421.61 L215.11,420.79 L215.91,419.8 L224.0,418.29 L228.53,405.26 L230.2,396.51 L230.25,395.81 L225.38,397.31 L224.16,397.18 L223.12,396.63 L221.21,393.64 L221.15,391.94 L220.5,390.94 L221.02,390.38 L221.07,389.26 L226.75,388.15 L228.24,387.49 L239.41,386.23 L244.51,386.41 L246.68,387.08 L247.83,388.66 L249.39,389.32 L249.79,391.8 L248.14,392.8 L245.46,393.68 L238.57,394.69 L235.73,409.91 L233.41,416.78 L236.98,416.53 L237.39,417.22 L239.12,417.94 L239.51,418.68 L240.71,419.22 L241.0,420.71 L239.91,422.94 L239.17,423.12 L238.3,424.05 L232.58,424.66 L232.31,425.32 L230.07,425.05 L227.8,425.31 L220.74,427.31ZM248.66,414.24 L247.19,413.25 L245.77,411.36 L244.99,409.33 L245.06,407.7 L246.11,405.94 L247.89,404.8 L249.29,404.31 L251.26,405.33 L251.56,405.15 L252.84,401.42 L253.0,399.92 L250.89,399.18 L248.88,396.75 L248.03,396.2 L248.39,395.37 L248.01,394.13 L248.08,393.08 L249.09,391.69 L259.54,387.42 L261.4,387.05 L268.4,384.6 L273.27,383.6 L276.49,383.37 L277.93,383.64 L282.56,387.18 L282.99,388.4 L282.94,389.64 L282.54,390.28 L280.93,391.1 L276.17,391.51 L270.59,392.6 L265.26,394.22 L262.12,395.85 L262.0,398.61 L261.28,400.1 L261.16,401.78 L260.59,403.23 L260.92,404.39 L268.59,402.82 L273.26,402.37 L272.82,402.87 L273.4,402.88 L273.14,403.75 L274.07,404.11 L274.82,404.88 L275.31,405.98 L272.65,410.29 L270.83,410.32 L257.56,413.77 L256.37,416.45 L256.49,417.39 L255.98,417.58 L255.57,419.51 L256.0,421.14 L255.83,422.57 L255.12,424.0 L254.01,425.02 L252.92,425.08 L250.86,424.55 L250.19,424.07 L248.21,421.4 L247.83,418.77ZM288.03,423.96 L286.57,423.75 L284.2,422.68 L283.3,421.82 L282.06,419.46 L283.0,415.33 L283.85,414.0 L287.54,405.54 L285.62,403.3 L283.17,398.34 L281.47,392.12 L281.63,390.57 L280.61,386.74 L280.86,384.55 L282.46,382.9 L288.53,385.76 L288.67,386.74 L289.36,386.97 L289.58,388.92 L290.15,390.03 L291.49,394.87 L291.59,396.27 L292.69,395.3 L296.02,390.04 L295.87,389.42 L298.18,386.96 L300.64,383.59 L302.58,382.15 L305.75,380.96 L306.7,382.64 L309.52,384.29 L310.2,386.41 L309.87,387.36 L309.09,388.68 L304.7,393.61 L304.02,395.56 L303.14,395.68 L301.56,398.22 L298.87,403.54 L299.01,404.11 L298.14,405.11 L294.49,412.04 L293.61,414.73 L291.87,417.66 L290.41,421.67 L288.9,423.58Z\" fill=\"#f5b335\"/></svg></div>\
<h1>{heading}</h1><p>{body}</p></main><script>setTimeout(function(){{window.close()}},1500)</script></html>"
    )
}

fn success_page() -> String {
    page(
        "Signed in to Leaximy",
        "You're signed in",
        "You can close this tab and go back to Leaximy.",
        "#f5b335",
    )
}

fn failure_page(reason: &str) -> String {
    page(
        "Sign-in failed",
        "Sign-in didn't complete",
        &format!("{reason}. Return to Leaximy and try again."),
        "#f5717f",
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn token_errors_never_include_authorization_response_contents() {
        for (status, body) in [
            (
                400,
                r#"{"error":"invalid_grant","error_description":"dummy-refresh-secret"}"#,
            ),
            (400, r#"{"error":"dummy-refresh-secret"}"#),
            (503, "dummy-refresh-secret"),
            (
                200,
                r#"{"access_token":"dummy-refresh-secret","expires_in":"dummy-refresh-secret"}"#,
            ),
        ] {
            let error =
                super::decode_token_response(reqwest::StatusCode::from_u16(status).unwrap(), body)
                    .unwrap_err();
            assert!(!format!("{error:?} {error}").contains("dummy-refresh-secret"));
        }
        for request in [
            "GET /login?error=dummy-authorization-secret&state=ok HTTP/1.1",
            "GET /dummy-authorization-secret HTTP/1.1",
        ] {
            let error = super::parse_request_line(request, "ok").unwrap_err();
            assert!(!format!("{error:?} {error}").contains("dummy-authorization-secret"));
        }
    }

    use super::*;

    #[test]
    fn old_web_grants_require_image_upload_consent() {
        let token = StoredToken {
            scope: WEB_SCOPES
                .iter()
                .filter(|scope| **scope != "ugc-image-upload")
                .copied()
                .collect::<Vec<_>>()
                .join(" "),
            ..Default::default()
        };
        assert!(!token.has_scopes(WEB_SCOPES));
        let renewed = StoredToken {
            scope: WEB_SCOPES.join(" "),
            ..token
        };
        assert!(renewed.has_scopes(WEB_SCOPES));
    }

    #[test]
    fn begin_produces_valid_pkce_material() {
        let flow = begin(Grant::shared_web_api());
        assert!(flow.verifier.len() >= 43);
        assert!(flow.url.contains("code_challenge_method=S256"));
        assert!(flow.url.contains(&format!("state={}", flow.state)));
        assert!(
            flow.url
                .contains(&format!("client_id={DEFAULT_WEB_CLIENT_ID}"))
        );
        assert!(flow.url.contains("8989"));
        assert!(!flow.url.contains("show_dialog=true"));
        let playback = begin(Grant::playback());
        assert!(
            playback
                .url
                .contains(&format!("client_id={PLAYBACK_CLIENT_ID}"))
        );
        assert!(playback.url.contains("8898"));
        assert!(playback.url.contains("show_dialog=true"));
    }

    #[test]
    fn playback_scopes_conform_to_spotify_streaming_contract() {
        assert_eq!(PLAYBACK_SCOPES, &["streaming"]);
        let playback = Grant::playback();
        assert_eq!(playback.scopes, vec!["streaming".to_string()]);
        let personal = Grant::personal_web_api("test-id").unwrap();
        let flow = begin(personal);
        assert!(!flow.url.contains("show_dialog=true"));
    }

    #[test]
    fn personal_client_id_is_validated_at_the_boundary() {
        assert_eq!(Grant::personal_web_api("  abc ").unwrap().client_id, "abc");
        assert!(Grant::personal_web_api("  ").is_err());
    }

    #[test]
    fn request_line_parsing() {
        let code =
            parse_request_line("GET /login?code=abc%20d&state=s1 HTTP/1.1\r\n", "s1").unwrap();
        assert_eq!(code, "abc d");
        assert!(parse_request_line("GET /login?code=abc&state=other HTTP/1.1", "s1").is_err());
        assert!(parse_request_line("GET /favicon.ico HTTP/1.1", "s1").is_err());
        assert!(
            parse_request_line("GET /login?error=access_denied&state=s1 HTTP/1.1", "s1").is_err()
        );
    }

    #[test]
    fn stored_token_round_trips_and_tracks_expiry() {
        let response = TokenResponse {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_in: Some(3600),
            scope: Some("x".into()),
        };
        let token = StoredToken::from_response("id", response, None).unwrap();
        assert!(!token.needs_refresh());
        assert!(token.has_scopes(&["x"]));
        assert!(!token.has_scopes(&["x", "y"]));
        let expired = StoredToken {
            expires_at: now_secs() + 10,
            ..token.clone()
        };
        assert!(expired.needs_refresh());
        let dir = std::env::temp_dir().join(format!("leaximy-token-{}", std::process::id()));
        let path = dir.join("token.json");
        token.save(&path).unwrap();
        assert_eq!(StoredToken::load(&path), Some(token));
        StoredToken::remove(&path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_tokens_move_to_the_matching_session() {
        let dir = std::env::temp_dir().join(format!(
            "leaximy-token-migration-{}-{}",
            std::process::id(),
            now_secs()
        ));
        let legacy = dir.join("legacy.json");
        let shared = dir.join("shared.json");
        let personal = dir.join("personal.json");
        for (client_id, target) in [
            (DEFAULT_WEB_CLIENT_ID, &shared),
            ("personal-client", &personal),
        ] {
            let token = StoredToken {
                client_id: client_id.into(),
                access_token: "access".into(),
                refresh_token: "refresh".into(),
                expires_at: now_secs() + 3600,
                ..StoredToken::default()
            };
            token.save(&legacy).unwrap();
            StoredToken::migrate_legacy(&legacy, &shared, &personal).unwrap();
            assert_eq!(StoredToken::load(target), Some(token));
            assert!(!legacy.exists());
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
