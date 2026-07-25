//! # 备份文件加密辅助
//!
//! 远端备份（WebDAV）支持 AES-256-GCM 加密。
//! 密钥与 `secret` 模块共享：由用户在「设置」中配置的 1-20 位通用密码
//! 经 SHA-256 哈希得到 32 字节。
//!
//! 密文格式：`nonce(12B) || ciphertext_with_tag(N+16B)`

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::secret::crypto::derive_key_from_password;

/// GCM nonce 长度（字节）
const NONCE_LEN: usize = 12;

/// 加密备份文件字节
///
/// 返回 `nonce(12B) || ciphertext_with_tag` 拼接后的字节。
pub fn encrypt_backup(password: &str, plaintext: &[u8]) -> IcodeResult<Vec<u8>> {
    let key = derive_key_from_password(password);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| IcodeError::internal(format!("备份加密失败: {e}")))?;

    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// 解密备份文件字节
///
/// 输入应为 `encrypt_backup()` 产出的 `nonce(12B) || ciphertext_with_tag`。
pub fn decrypt_backup(password: &str, encrypted: &[u8]) -> IcodeResult<Vec<u8>> {
    if encrypted.len() <= NONCE_LEN {
        return Err(IcodeError::validation(format!(
            "加密备份文件长度异常：期望大于 {NONCE_LEN} 字节"
        )));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_LEN);
    let key = derive_key_from_password(password);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| IcodeError::validation("备份解密失败：密码错误或文件损坏"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_encrypt_decrypt_roundtrip() {
        let password = "my-backup-pwd";
        let plaintext = b"hello zip bytes";
        let encrypted = encrypt_backup(password, plaintext).unwrap();
        assert!(encrypted.len() > NONCE_LEN);
        let decrypted = decrypt_backup(password, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_password_fails() {
        let encrypted = encrypt_backup("correct", b"data").unwrap();
        assert!(decrypt_backup("wrong", &encrypted).is_err());
    }
}
