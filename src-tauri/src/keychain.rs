// Flick - keychain.rs
// Per PRD §8.7: API key stored in OS native keychain, never in plaintext config.
// Uses keyring crate with service "flick" and username "gemini_api_key".

use anyhow::{Context, Result};

const SERVICE: &str = "flick";
const LEGACY_USERNAME: &str = "gemini_api_key";

fn username(provider: &str) -> String {
    let provider = provider
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .collect::<String>();
    format!(
        "{}_api_key",
        if provider.is_empty() {
            "gemini"
        } else {
            &provider
        }
    )
}

/// Save the Gemini API key to the OS keychain.
pub fn save_api_key(provider: &str, key: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, &username(provider))
        .context("Failed to create keyring entry")?;
    entry
        .set_password(key)
        .context("Failed to save API key to keychain")?;
    log::info!("API key saved to OS keychain");
    Ok(())
}

/// Load the Gemini API key from the OS keychain.
pub fn load_api_key(provider: &str) -> Result<String> {
    let entry = keyring::Entry::new(SERVICE, &username(provider))
        .context("Failed to create keyring entry")?;
    match entry.get_password() {
        Ok(key) => Ok(key),
        // Flick 1.x stored the default credential under this fixed name.
        // Read it for the default provider so existing users do not need to
        // enter a key again; the next save writes the scoped credential.
        Err(_) if provider == "gemini" => keyring::Entry::new(SERVICE, LEGACY_USERNAME)
            .context("Failed to create legacy keyring entry")?
            .get_password()
            .context("No API key found in keychain"),
        Err(error) => Err(error).context("No API key found in keychain"),
    }
}

/// Delete the Gemini API key from the OS keychain.
pub fn delete_api_key(provider: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, &username(provider))
        .context("Failed to create keyring entry")?;
    entry
        .delete_credential()
        .context("Failed to delete API key from keychain")?;
    log::info!("API key deleted from OS keychain");
    Ok(())
}
