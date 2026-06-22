// Flick — keychain.rs
// Per PRD §8.7: API key stored in OS native keychain, never in plaintext config.
// Uses keyring crate with service "flick" and username "gemini_api_key".

use anyhow::{Context, Result};

const SERVICE: &str = "flick";
const USERNAME: &str = "gemini_api_key";

/// Save the Gemini API key to the OS keychain.
pub fn save_api_key(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, USERNAME)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(key)
        .context("Failed to save API key to keychain")?;
    log::info!("API key saved to OS keychain");
    Ok(())
}

/// Load the Gemini API key from the OS keychain.
pub fn load_api_key() -> Result<String> {
    let entry = keyring::Entry::new(SERVICE, USERNAME)
        .context("Failed to create keyring entry")?;
    let key = entry
        .get_password()
        .context("No API key found in keychain")?;
    Ok(key)
}

/// Delete the Gemini API key from the OS keychain.
pub fn delete_api_key() -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, USERNAME)
        .context("Failed to create keyring entry")?;
    entry
        .delete_credential()
        .context("Failed to delete API key from keychain")?;
    log::info!("API key deleted from OS keychain");
    Ok(())
}
