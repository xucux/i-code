//! # AES-GCM 加密原语封装
//!
//! 提供基于 AES-256-GCM 的对称加密与解密能力，用于本地加密存储敏感凭据。
//!
//! ## 主密钥管理
//!
//! v0.2 实现方案：
//! - 主密钥不再随机生成，而是**由用户在「设置」中输入的 1-20 位通用密码经 SHA-256 派生**。
//! - 该密码同时用于：
//!   1. 加密/解密 API Key、Token 等敏感数据（通过 `secret` 模块）。
//!   2. 加密/解密远端 WebDAV 备份文件（通过 `backup` 模块）。
//! - 密码本身以明文形式保存在 `app_settings.config_key` 列，便于派生密钥。
//!   若后续需要更高安全性，可引入 OS 密钥链或 Tauri Stronghold 保护该密码。
//!
//! **安全限制**：此方案适用于本地桌面端单用户场景。
//! 生产环境建议改用 Tauri `stronghold` 插件或 OS 密钥链存储主密钥，
//! 详见 `docs/proposals/secret-storage.md`。
//!
//! ## 密文格式
//!
//! `encrypted_value` 列存储格式：`nonce(12B) || ciphertext_with_tag(N+16B)`
//! - nonce：每次加密生成 12 字节随机数（GCM 标准长度）
//! - ciphertext_with_tag：明文长度 + 16 字节 GCM 认证标签

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::{IcodeError, IcodeResult};

/// AES-256-GCM 主密钥长度（字节）
const MASTER_KEY_LEN: usize = 32;

/// GCM nonce 长度（字节）
const NONCE_LEN: usize = 12;

/// AES-256-GCM 主密钥字节数组
pub type MasterKey = [u8; MASTER_KEY_LEN];

/// 从用户通用密码派生 32 字节 AES-256 主密钥
///
/// 使用 SHA-256(password) 直接作为密钥，符合「密码取 hash 做密钥」的需求。
/// 注意：修改通用密码后，此前用旧密码加密的 Secret 与备份文件将无法解密。
pub fn derive_key_from_password(password: &str) -> MasterKey {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; MASTER_KEY_LEN];
    key.copy_from_slice(&result);
    key
}

/// 加密明文为二进制密文
///
/// 返回 `nonce(12B) || ciphertext_with_tag(N+16B)` 拼接字节数组
///
/// # 参数
/// - `master_key`：主密钥
/// - `plaintext`：待加密的明文字符串
pub fn encrypt(master_key: &MasterKey, plaintext: &str) -> IcodeResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key));

    // 生成随机 nonce（每次加密独立生成，避免重用）
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 加密：ciphertext 包含 16 字节 GCM 认证标签
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| IcodeError::internal(format!("AES-GCM 加密失败: {e}")))?;

    // 拼接 nonce + ciphertext
    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// 解密密文为明文字符串
///
/// 输入应为 `encrypt()` 产出的 `nonce(12B) || ciphertext_with_tag` 拼接字节数组
///
/// # 参数
/// - `master_key`：主密钥
/// - `encrypted_value`：密文字节数组
///
/// # 错误
/// - 输入过短（小于 nonce 长度）：返回 `VALIDATION` 错误
/// - 解密失败（标签校验失败、密钥错误等）：返回 `INTERNAL` 错误
pub fn decrypt(master_key: &MasterKey, encrypted_value: &[u8]) -> IcodeResult<String> {
    if encrypted_value.len() <= NONCE_LEN {
        return Err(IcodeError::validation(format!(
            "密文长度异常：期望大于 {NONCE_LEN} 字节，实际 {} 字节",
            encrypted_value.len()
        )));
    }

    let (nonce_bytes, ciphertext) = encrypted_value.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key));
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| IcodeError::internal("AES-GCM 解密失败：密钥错误或密文损坏"))?;

    String::from_utf8(plaintext)
        .map_err(|e| IcodeError::internal(format!("解密后的明文不是有效 UTF-8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_master_key() -> MasterKey {
        derive_key_from_password("test-password-123")
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_master_key();
        let plaintext = "sk-1234567890abcdef";
        let ciphertext = encrypt(&key, plaintext).unwrap();
        assert_ne!(ciphertext, plaintext.as_bytes());
        assert!(ciphertext.len() > NONCE_LEN);

        let decrypted = decrypt(&key, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        // 相同明文两次加密应得到不同密文（因 nonce 随机）
        let key = test_master_key();
        let plaintext = "sk-same-value";
        let c1 = encrypt(&key, plaintext).unwrap();
        let c2 = encrypt(&key, plaintext).unwrap();
        assert_ne!(c1, c2, "相同明文应得到不同密文");

        // 但都能正确解密
        assert_eq!(decrypt(&key, &c1).unwrap(), plaintext);
        assert_eq!(decrypt(&key, &c2).unwrap(), plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let key1 = derive_key_from_password("correct");
        let key2 = derive_key_from_password("wrong");

        let ciphertext = encrypt(&key1, "secret").unwrap();
        let result = decrypt(&key2, &ciphertext);
        assert!(result.is_err(), "使用错误密钥应解密失败");
    }

    #[test]
    fn test_decrypt_tampered_ciphertext_fails() {
        let key = test_master_key();
        let mut ciphertext = encrypt(&key, "secret").unwrap();
        // 篡改最后一个字节
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xff;
        let result = decrypt(&key, &ciphertext);
        assert!(result.is_err(), "密文被篡改应解密失败");
    }

    #[test]
    fn test_decrypt_too_short_input() {
        let key = test_master_key();
        let short = vec![0u8; 5];
        assert!(decrypt(&key, &short).is_err());
    }

    #[test]
    fn test_derive_key_deterministic() {
        // 相同密码应派生相同密钥
        let key1 = derive_key_from_password("same");
        let key2 = derive_key_from_password("same");
        assert_eq!(key1, key2);

        // 不同密码应派生不同密钥
        let key3 = derive_key_from_password("different");
        assert_ne!(key1, key3);
    }
}
