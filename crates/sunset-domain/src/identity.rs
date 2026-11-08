//! The app's public identity: its product name and bundle id.
//!
//! Both values are temporary until the product is named. They are written once, in
//! `app-identity.json` at the repo root, and reach Rust through the app shell's runtime config.
//! Code never hard-codes them; it receives an [`AppIdentity`] instead.
//! See `docs/architecture.md` §2.7.

use thiserror::Error;

/// Product name and bundle id, validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    product_name: String,
    bundle_id: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error("product name must not be empty")]
    EmptyProductName,
    #[error("bundle id {0:?} must be reverse-DNS, e.g. com.example.app (letters, digits, '-')")]
    InvalidBundleId(String),
}

impl AppIdentity {
    pub fn new(
        product_name: impl Into<String>,
        bundle_id: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        let product_name = product_name.into();
        let bundle_id = bundle_id.into();
        if product_name.trim().is_empty() {
            return Err(IdentityError::EmptyProductName);
        }
        if !is_valid_bundle_id(&bundle_id) {
            return Err(IdentityError::InvalidBundleId(bundle_id));
        }
        Ok(Self {
            product_name,
            bundle_id,
        })
    }

    /// Name shown to the user: window title, menu bar, Dock.
    pub fn product_name(&self) -> &str {
        &self.product_name
    }

    /// Reverse-DNS id. macOS uses it for the app data directory and preferences.
    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    /// Service name for API keys in the macOS Keychain.
    pub fn keychain_service(&self) -> &str {
        &self.bundle_id
    }

    /// Suggested folder name for the Markdown vault mirror. The user can pick another.
    pub fn default_vault_folder_name(&self) -> String {
        format!("{} Vault", self.product_name)
    }
}

/// Same rule as `scripts/identity.mjs`: at least two dot-separated segments, each starting with
/// a letter and containing only ASCII letters, digits and '-'.
fn is_valid_bundle_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|s| {
            s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_keychain_service_and_vault_name() {
        let id = AppIdentity::new("Sunset", "dev.example.sunset").unwrap();
        assert_eq!(id.keychain_service(), "dev.example.sunset");
        assert_eq!(id.default_vault_folder_name(), "Sunset Vault");
    }

    #[test]
    fn rejects_bad_values() {
        assert_eq!(
            AppIdentity::new(" ", "dev.example.sunset"),
            Err(IdentityError::EmptyProductName)
        );
        for bad in [
            "sunset",
            "dev..sunset",
            "dev.example.my_app",
            "1dev.example",
            "",
        ] {
            assert!(
                AppIdentity::new("Sunset", bad).is_err(),
                "{bad} should fail"
            );
        }
    }
}
