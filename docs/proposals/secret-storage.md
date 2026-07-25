# 敏感数据存储模式方案对比

> 状态：v0.1 已采用方案 A（本地 AES-GCM），待后续迭代选择是否切换。
> 关联：`docs/development.md` §5.9 secret、`docs/database.md` §4.2 secrets 表

## 背景

应用需要安全存储 API Key、OAuth Token、代理认证、Gateway Key 等敏感凭据，
用于网关转发请求时还原为明文。存储方案需在「安全性 / 跨设备迁移 / 实现成本」三者间平衡。

`app_settings.store_secrets_in_keychain` 字段预留了切换开关：

- `0`：本地 AES-GCM 加密（v0.1 实现）
- `1`：系统密钥链（待实现）

## 方案 A：本地 AES-GCM 加密（v0.1 已实现）

**实现位置**：`src-tauri/src/modules/secret/crypto.rs`

### 设计要点

- 主密钥：32 字节随机数，存储在应用数据目录的 `master.key` 文件中
- 加密算法：AES-256-GCM，每条 Secret 独立 12 字节随机 nonce
- 密文格式：`nonce(12B) || ciphertext_with_tag(N+16B)`
- 主密钥文件权限：Unix 0600，Windows 默认 ACL
- 主密钥写入采用原子操作（先写 `.tmp` 再 rename）

### 优点

- 实现简单，跨平台一致
- 不依赖系统 API，Tauri 全平台支持
- 数据库文件可整体备份（密钥与密文同目录）

### 缺点

- **主密钥保护薄弱**：任何能读取 `master.key` 文件的进程都可解密全部 Secret
- 无法与系统解锁凭据关联（不要求用户登录）
- 跨设备迁移时密钥与密文必须成对迁移

### 适用场景

- v0.1 快速验证阶段
- 个人单机使用场景

## 方案 B：系统密钥链直接存储

**依赖**：`tauri-plugin-stronghold` 或 `keytar` crate

### 设计要点

- 每条 Secret 直接存入系统密钥链：
  - macOS：Keychain
  - Windows：Credential Manager
  - Linux：libsecret / KWallet / GNOME Keyring
- `secrets.encrypted_value` 列存储密钥链中的 key（如 `i-code:secret:{uuid}`）
- `master.key` 文件不再需要

### 优点

- 主密钥不落盘，由系统保护
- 用户可单独锁屏加密
- 跨进程隔离：其他应用无法读取

### 缺点

- **跨设备迁移复杂**：密钥链数据无法随 SQLite 文件一起备份
- Linux 依赖桌面环境（无 GUI 的服务器不可用）
- `tauri-plugin-stronghold` 引入 IOTA Stronghold，二进制体积增加
- `keytar` 在 Linux 上依赖 libsecret，需额外系统库
- 容量限制：单条 Secret 大小受系统限制

### 适用场景

- 桌面端优先，不需要跨设备迁移
- Linux 桌面环境完整

## 方案 C：混合模式（密钥链保护主密钥 + AES-GCM 加密数据）

### 设计要点

- 主密钥仍为 32 字节随机数，但不存为文件
- 主密钥存入系统密钥链（一项条目），AES-GCM 加密后的密文存入 SQLite
- 备份时：SQLite 文件单独备份，主密钥由用户在目标设备重新设置（或导出加密包）

### 优点

- 兼顾性能（密钥链读取一次后内存缓存）与安全性
- 跨设备迁移：迁移 SQLite，由用户在新设备重新设置主密钥（触发解密失败后引导流程）
- 单条 Secret 不受密钥链容量限制

### 缺点

- 实现复杂度高于方案 A 与方案 B
- 主密钥丢失即数据不可恢复（需提供重置流程）
- Linux 桌面环境依赖

### 适用场景

- 平衡安全与迁移便利
- 中长期推荐方案

## 方案 D：用户口令派生主密钥（PBKDF2 + AES-GCM）

### 设计要点

- 用户首次启动设置主口令
- 通过 PBKDF2（已有依赖）从口令派生 32 字节密钥
- 派生时使用随机 salt（存入 `master.salt` 文件）
- 主密钥仅在用户解锁后驻留内存

### 优点

- 无需系统密钥链依赖，跨平台一致
- 主密钥不落盘，安全性最高
- 跨设备迁移：用户在新设备输入相同口令即可解锁（SQLite 随备份迁移）

### 缺点

- 用户体验差：每次启动需输入口令
- 口令丢失不可恢复
- 与 Tauri 的常驻进程模式冲突（迷你面板需保持运行）

### 适用场景

- 高安全需求场景
- 单机企业部署

## 选型建议

| 维度 | 方案 A | 方案 B | 方案 C | 方案 D |
|------|--------|--------|--------|--------|
| 实现成本 | 低 ✓ | 中 | 中高 | 中 |
| 安全性 | 中 | 高 | 高 ✓ | 最高 |
| 跨设备迁移 | 简单 ✓ | 复杂 | 中 | 简单 ✓ |
| Linux 兼容 | 完整 ✓ | 依赖桌面环境 | 依赖桌面环境 | 完整 ✓ |
| 用户体验 | 无感 ✓ | 无感 ✓ | 无感 ✓ | 需输入口令 ✗ |

### v0.1 决策

采用**方案 A**：实现简单、跨平台一致、无需额外依赖，满足快速验证需求。

### v0.2+ 演进建议

- **优先方案 C**：当 `store_secrets_in_keychain = 1` 时切换到混合模式，
  主密钥存入系统密钥链，密文仍存 SQLite。
- 引入 `tauri-plugin-stronghold`（已列入 `Cargo.toml` 注释）作为跨平台密钥链抽象层。
- 提供「重置主密钥」流程（旧密钥解密 → 新密钥加密 → 一次性事务写入）。

## 实现差异点

切换到方案 C 时需修改的代码点：

1. `modules/secret/crypto.rs`：新增 `load_from_keychain()` / `save_to_keychain()` 替代 `load_or_create_master_key()`
2. `modules/secret/service.rs`：`SecretServiceHandle::new()` 改为从密钥链加载主密钥
3. `main.rs`：启动期初始化时根据 `app_settings.store_secrets_in_keychain` 选择存储后端
4. 备份模块：需要单独处理主密钥的导出/导入（加密包形式）

## 参考实现

- Tauri Stronghold 插件文档：https://v2.tauri.app/plugin/stronghold/
- 1Password 类似架构：本地加密 + 同步，密钥不落盘
- VSCode SecretStorage：采用 keytar 跨平台抽象
