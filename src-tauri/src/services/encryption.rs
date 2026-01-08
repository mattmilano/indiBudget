use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Argon2, Algorithm, Params, Version};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 32;
const KEY_SIZE: usize = 32;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Encryption not enabled")]
    NotEnabled,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub enabled: bool,
    pub salt: Option<Vec<u8>>,
    pub verification_hash: Option<Vec<u8>>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            salt: None,
            verification_hash: None,
        }
    }
}

pub struct EncryptionService {
    config: EncryptionConfig,
    key: Option<[u8; KEY_SIZE]>,
    config_path: PathBuf,
}

impl EncryptionService {
    pub fn new(data_dir: PathBuf) -> Result<Self, EncryptionError> {
        let config_path = data_dir.join("encryption.json");

        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)
                .map_err(|e| EncryptionError::Serialization(e.to_string()))?
        } else {
            EncryptionConfig::default()
        };

        Ok(Self {
            config,
            key: None,
            config_path,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn is_unlocked(&self) -> bool {
        self.key.is_some()
    }

    pub fn enable(&mut self, password: &str) -> Result<(), EncryptionError> {
        let mut rng = rand::thread_rng();
        let salt: Vec<u8> = (0..SALT_SIZE).map(|_| rng.gen()).collect();

        let key = derive_key(password, &salt)?;

        // Create a verification hash by encrypting a known value
        let verification_data = b"indiBudget_encryption_verification";
        let verification_hash = self.encrypt_bytes(verification_data, &key)?;

        self.config.enabled = true;
        self.config.salt = Some(salt);
        self.config.verification_hash = Some(verification_hash);
        self.key = Some(key);

        self.save_config()?;

        Ok(())
    }

    pub fn disable(&mut self, password: &str) -> Result<(), EncryptionError> {
        // Verify password first
        self.unlock(password)?;

        self.config.enabled = false;
        self.config.salt = None;
        self.config.verification_hash = None;
        self.key = None;

        self.save_config()?;

        Ok(())
    }

    pub fn unlock(&mut self, password: &str) -> Result<(), EncryptionError> {
        if !self.config.enabled {
            return Err(EncryptionError::NotEnabled);
        }

        let salt = self.config.salt.as_ref()
            .ok_or(EncryptionError::InvalidPassword)?;

        let key = derive_key(password, salt)?;

        // Verify the password by decrypting the verification hash
        let verification_hash = self.config.verification_hash.as_ref()
            .ok_or(EncryptionError::InvalidPassword)?;

        let decrypted = self.decrypt_bytes(verification_hash, &key)?;

        if decrypted != b"indiBudget_encryption_verification" {
            return Err(EncryptionError::InvalidPassword);
        }

        self.key = Some(key);

        Ok(())
    }

    pub fn lock(&mut self) {
        self.key = None;
    }

    pub fn change_password(&mut self, old_password: &str, new_password: &str) -> Result<(), EncryptionError> {
        // Verify old password
        self.unlock(old_password)?;

        // Generate new salt and key
        let mut rng = rand::thread_rng();
        let new_salt: Vec<u8> = (0..SALT_SIZE).map(|_| rng.gen()).collect();
        let new_key = derive_key(new_password, &new_salt)?;

        // Create new verification hash
        let verification_data = b"indiBudget_encryption_verification";
        let new_verification_hash = self.encrypt_bytes(verification_data, &new_key)?;

        self.config.salt = Some(new_salt);
        self.config.verification_hash = Some(new_verification_hash);
        self.key = Some(new_key);

        self.save_config()?;

        Ok(())
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, EncryptionError> {
        if !self.config.enabled {
            return Ok(plaintext.to_string());
        }

        let key = self.key.ok_or(EncryptionError::DecryptionFailed("Not unlocked".to_string()))?;
        let encrypted = self.encrypt_bytes(plaintext.as_bytes(), &key)?;

        // Return as base64
        Ok(base64_encode(&encrypted))
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String, EncryptionError> {
        if !self.config.enabled {
            return Ok(ciphertext.to_string());
        }

        let key = self.key.ok_or(EncryptionError::DecryptionFailed("Not unlocked".to_string()))?;
        let encrypted = base64_decode(ciphertext)
            .map_err(|_| EncryptionError::DecryptionFailed("Invalid base64".to_string()))?;

        let decrypted = self.decrypt_bytes(&encrypted, &key)?;

        String::from_utf8(decrypted)
            .map_err(|_| EncryptionError::DecryptionFailed("Invalid UTF-8".to_string()))
    }

    fn encrypt_bytes(&self, plaintext: &[u8], key: &[u8; KEY_SIZE]) -> Result<Vec<u8>, EncryptionError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; NONCE_SIZE] = rng.gen();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);

        Ok(result)
    }

    fn decrypt_bytes(&self, ciphertext: &[u8], key: &[u8; KEY_SIZE]) -> Result<Vec<u8>, EncryptionError> {
        if ciphertext.len() < NONCE_SIZE {
            return Err(EncryptionError::DecryptionFailed("Ciphertext too short".to_string()));
        }

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

        let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
        let ciphertext = &ciphertext[NONCE_SIZE..];

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }

    fn save_config(&self) -> Result<(), EncryptionError> {
        let content = serde_json::to_string_pretty(&self.config)
            .map_err(|e| EncryptionError::Serialization(e.to_string()))?;

        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.config_path, content)?;

        Ok(())
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE], EncryptionError> {
    let params = Params::new(
        65536,  // 64 MB memory
        3,      // 3 iterations
        4,      // 4 parallel lanes
        Some(KEY_SIZE),
    ).map_err(|e| EncryptionError::KeyDerivationFailed(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_SIZE];
    argon2.hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| EncryptionError::KeyDerivationFailed(e.to_string()))?;

    Ok(key)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.decode(data)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionStatus {
    pub enabled: bool,
    pub unlocked: bool,
}
