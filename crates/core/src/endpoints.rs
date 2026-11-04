use crate::auth::MsaEndpoints;

/// Base URLs of every remote service. Tests point these at a mock server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub mojang_meta: String,
    pub libraries: String,
    pub resources: String,
    pub adoptium: String,
    pub fabric_meta: String,
    pub quilt_meta: String,
    pub forge_maven: String,
    pub forge_files: String,
    pub neoforge_maven: String,
    pub msa: MsaEndpoints,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            mojang_meta: "https://piston-meta.mojang.com".into(),
            libraries: "https://libraries.minecraft.net".into(),
            resources: "https://resources.download.minecraft.net".into(),
            adoptium: "https://api.adoptium.net".into(),
            fabric_meta: "https://meta.fabricmc.net".into(),
            quilt_meta: "https://meta.quiltmc.org".into(),
            forge_maven: "https://maven.minecraftforge.net".into(),
            forge_files: "https://files.minecraftforge.net".into(),
            neoforge_maven: "https://maven.neoforged.net".into(),
            msa: MsaEndpoints::default(),
        }
    }
}

impl Endpoints {
    /// Every service at one base URL (a single mock server in tests).
    pub fn all_at(base: &str) -> Self {
        let b = base.trim_end_matches('/').to_string();
        Self {
            mojang_meta: b.clone(),
            libraries: b.clone(),
            resources: b.clone(),
            adoptium: b.clone(),
            fabric_meta: b.clone(),
            quilt_meta: b.clone(),
            forge_maven: b.clone(),
            forge_files: b.clone(),
            neoforge_maven: b.clone(),
            msa: MsaEndpoints {
                login: b.clone(),
                xbl: b.clone(),
                xsts: b.clone(),
                mc: b,
            },
        }
    }
}
