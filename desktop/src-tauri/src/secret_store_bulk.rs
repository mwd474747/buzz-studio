use std::collections::HashMap;

use super::SecretStore;

impl SecretStore {
    /// Merge a set of entries into the keychain blob in one mutation.
    pub fn store_all(&self, entries: &HashMap<String, String>) -> Result<(), String> {
        #[cfg(feature = "system-keyring")]
        {
            self.mutate_blob(|map| {
                for (key, value) in entries {
                    map.insert(key.clone(), value.clone());
                }
            })
        }
        #[cfg(not(feature = "system-keyring"))]
        {
            let _ = entries;
            Err("system-keyring feature disabled".to_string())
        }
    }
}
