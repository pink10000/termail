

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


