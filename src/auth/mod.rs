pub mod oauth;

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthProvider {
    ConfigFile,
    EnvironmentVariables,
    Keyring,
    Prompt,
    Oauth2,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Represents the authentication state for a backend
#[derive(Debug, Clone)]
pub enum AuthState {
    /// Not authenticated yet
    NotAuthenticated,
    /// Authenticated with username/password
    Authenticated(Credentials),
    /// Authenticated with OAuth2 token
    OAuth2(oauth::OAuth2Token),
}

impl AuthState {
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, AuthState::NotAuthenticated)
    }
}

