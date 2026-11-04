//! Accounts and sign-in (filled in by the auth task).

/// Base URLs of the Microsoft / Xbox / Minecraft sign-in services.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsaEndpoints {
    pub login: String,
    pub xbl: String,
    pub xsts: String,
    pub mc: String,
}

impl Default for MsaEndpoints {
    fn default() -> Self {
        Self {
            login: "https://login.microsoftonline.com".into(),
            xbl: "https://user.auth.xboxlive.com".into(),
            xsts: "https://xsts.auth.xboxlive.com".into(),
            mc: "https://api.minecraftservices.com".into(),
        }
    }
}
