use md5::{Digest, Md5};
use uuid::Uuid;

use crate::{AppError, Result};

/// Same as Java's `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`, which is
/// what servers expect for offline players.
pub fn offline_uuid(name: &str) -> Uuid {
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30; // version 3
    h[8] = (h[8] & 0x3f) | 0x80; // IETF variant
    Uuid::from_bytes(h)
}

/// Minecraft names: 3–16 characters, ASCII letters, digits and underscores.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = (3..=16).contains(&name.chars().count())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(AppError::new("error.auth.invalid_name").with_param("name", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java_name_uuid_from_bytes() {
        // Values computed with java.util.UUID.nameUUIDFromBytes on JDK 21.
        assert_eq!(
            offline_uuid("Notch").to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert_eq!(
            offline_uuid("玩家").to_string(),
            "511a6918-f669-3b23-ba72-decf60a9cb59"
        );
    }

    #[test]
    fn offline_uuid_is_v3_and_stable() {
        let a = offline_uuid("Alex");
        assert_eq!(a.get_version_num(), 3);
        assert_eq!(a, offline_uuid("Alex"));
        assert_ne!(a, offline_uuid("Steve2"));
    }

    #[test]
    fn offline_name_validation() {
        for bad in ["ab", "has space", "seventeen_chars_x", "玩家", ""] {
            assert_eq!(
                validate_name(bad).unwrap_err().key,
                "error.auth.invalid_name",
                "{bad}"
            );
        }
        for good in ["Player_01", "abc", "sixteen_chars_xx"] {
            validate_name(good).unwrap();
        }
    }
}
