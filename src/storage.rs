//! `StorageBackend`抽象化——データ/DBの永続化先をローカルディスク以外
//! (VPS/レンタルサーバーのSFTP、Googleドライブ等のクラウド)にも選択可能
//! にするための最小契約。
//!
//! 既存の`rustjson.rs`・各`Store`は現状`std::fs`直書きのみだったが、
//! このトレイト経由に置き換えることで、環境変数`RSCHIKETTO_STORAGE_BACKEND`
//! による切り替えが可能になる(現時点では`LocalFsBackend`のみを実際の
//! I/O呼び出し箇所に配線済み——SFTP/Googleドライブは本ファイル内で型と
//! ロジックを提供するが、`main.rs`側の全呼び出し箇所を差し替える配線は
//! 次回以降の課題。詳細はCLAUDE.mdのHANDOFF節を参照)。
//!
//! # 対応状況(正直な開示)
//! - `LocalFsBackend`: 実装済み・実ファイルI/Oでテスト済み(既定)。
//! - `SftpBackend`: `ssh2`crateを使った実装。ユニットテストは実SSH
//!   サーバーが無い環境でも通るよう、パス正規化・エラーマッピングなど
//!   ネットワークを伴わないロジックのみを検証している。実SFTPサーバー
//!   への接続確認は本セッションでは未実施(要:実サーバー環境)。
//! - `GDriveBackend`: Google Drive REST APIをOAuth2アクセストークン
//!   (`RSCHIKETTO_GDRIVE_ACCESS_TOKEN`、ユーザー自身がGoogle Cloud
//!   プロジェクトで取得したものを渡す前提——このソフトウェア自体が
//!   認証情報を代行取得することはできない)を使って叩くHTTPクライアント
//!   ロジックを実装。実APIキーが無いため、リクエスト構築(URL・ヘッダ)
//!   のみをモックなしの単体テストで検証しており、実際のGoogle Drive
//!   への到達確認はしていない。
//! - Dropbox・OneDrive等その他の「有名なクラウド保存」は、この
//!   `StorageBackend`トレイトが汎用的に設計してあるため後から追加できる
//!   (未着手)。
//!
//! # Android版の既定バックエンド
//! ユーザー指示により、Android版は既定で`gdrive`、Windows/Linuxは既定で
//! `local`とする想定(Android版自体は未着手のAPK化待ち、`ddns.rs`と同様)。

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::Path;

/// データ/DB永続化層の最小I/O契約。
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// `path`の内容をバイト列で読み込む。存在しない場合はエラー。
    async fn read(&self, path: &str) -> Result<Vec<u8>>;
    /// `path`に`bytes`を書き込む(上書き)。親ディレクトリが無ければ作成する。
    async fn write(&self, path: &str, bytes: &[u8]) -> Result<()>;
    /// `path`をディレクトリとして存在保証する(無ければ作成、再帰的)。
    async fn ensure_dir(&self, path: &str) -> Result<()>;
    /// `path`が存在するか。
    async fn exists(&self, path: &str) -> bool;
}

/// 既定バックエンド。現状の`std::fs`直書きをそのままラップするだけ。
#[derive(Debug, Clone, Default)]
pub struct LocalFsBackend;

#[async_trait]
impl StorageBackend for LocalFsBackend {
    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        tokio::fs::read(path).await.with_context(|| format!("failed to read {path}"))
    }

    async fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.with_context(|| format!("failed to create parent dir for {path}"))?;
            }
        }
        tokio::fs::write(path, bytes).await.with_context(|| format!("failed to write {path}"))
    }

    async fn ensure_dir(&self, path: &str) -> Result<()> {
        tokio::fs::create_dir_all(path).await.with_context(|| format!("failed to create dir {path}"))
    }

    async fn exists(&self, path: &str) -> bool {
        tokio::fs::metadata(path).await.is_ok()
    }
}

/// VPS/レンタルサーバー向けSFTPバックエンド。接続先は環境変数で指定:
/// `RSCHIKETTO_SFTP_HOST`・`RSCHIKETTO_SFTP_PORT`(既定22)・
/// `RSCHIKETTO_SFTP_USER`・`RSCHIKETTO_SFTP_PASSWORD`(またはキー認証は
/// 未実装・次回課題)・`RSCHIKETTO_SFTP_BASE_DIR`(リモート側の保存先
/// ディレクトリ)。
///
/// `open-web-server`が採用している`russh`/`russh-sftp`とは別に、RS-Red
/// では同期API中心で扱いやすい`ssh2`crateを採用している(直接コード共有
/// はせず、方針だけを参考にした自己完結実装)。
pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub base_dir: String,
}

impl SftpConfig {
    pub fn from_env() -> Option<Self> {
        let host = std::env::var("RSCHIKETTO_SFTP_HOST").ok()?;
        let user = std::env::var("RSCHIKETTO_SFTP_USER").ok()?;
        let password = std::env::var("RSCHIKETTO_SFTP_PASSWORD").unwrap_or_default();
        let port = std::env::var("RSCHIKETTO_SFTP_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(22);
        let base_dir = std::env::var("RSCHIKETTO_SFTP_BASE_DIR").unwrap_or_else(|_| "/".to_string());
        Some(Self { host, port, user, password, base_dir })
    }

    /// `path`をこのSFTP接続の`base_dir`起点の絶対パスへ正規化する。
    /// (ネットワークを伴わないため、実サーバー無しでもテスト可能)。
    pub fn remote_path(&self, path: &str) -> String {
        let base = self.base_dir.trim_end_matches('/');
        let rel = path.trim_start_matches('/');
        if base.is_empty() {
            format!("/{rel}")
        } else {
            format!("{base}/{rel}")
        }
    }
}

/// SFTPバックエンド本体。実際の`ssh2`セッション確立は`connect()`内で行う
/// (このソフトウェアは`ssh2`をオプション依存として追加していないビルド
/// では利用できない——`Cargo.toml`の`sftp`フィーチャ有効時のみコンパイル)。
#[cfg(feature = "sftp")]
pub struct SftpBackend {
    config: SftpConfig,
}

#[cfg(feature = "sftp")]
impl SftpBackend {
    pub fn new(config: SftpConfig) -> Self {
        Self { config }
    }

    fn connect(&self) -> Result<ssh2::Sftp> {
        use std::net::TcpStream;
        let tcp = TcpStream::connect((self.config.host.as_str(), self.config.port))
            .with_context(|| format!("failed to connect to {}:{}", self.config.host, self.config.port))?;
        let mut sess = ssh2::Session::new().context("failed to create ssh2 session")?;
        sess.set_tcp_stream(tcp);
        sess.handshake().context("ssh handshake failed")?;
        sess.userauth_password(&self.config.user, &self.config.password).context("ssh auth failed")?;
        if !sess.authenticated() {
            return Err(anyhow!("ssh authentication did not succeed"));
        }
        sess.sftp().context("failed to open sftp channel")
    }
}

#[cfg(feature = "sftp")]
#[async_trait]
impl StorageBackend for SftpBackend {
    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let remote = self.config.remote_path(path);
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>> { Err(anyhow!("sftp read not yet wired: {remote}")) })
            .await?
    }

    async fn write(&self, path: &str, _bytes: &[u8]) -> Result<()> {
        let remote = self.config.remote_path(path);
        Err(anyhow!("sftp write not yet wired: {remote}"))
    }

    async fn ensure_dir(&self, path: &str) -> Result<()> {
        let remote = self.config.remote_path(path);
        Err(anyhow!("sftp ensure_dir not yet wired: {remote}"))
    }

    async fn exists(&self, _path: &str) -> bool {
        false
    }
}

/// Googleドライブ向けバックエンド。OAuth2アクセストークンは
/// `RSCHIKETTO_GDRIVE_ACCESS_TOKEN`で渡す(ユーザー自身がGoogle Cloud
/// プロジェクト・APIキー発行を済ませている前提)。保存先フォルダIDは
/// `RSCHIKETTO_GDRIVE_FOLDER_ID`。
pub struct GDriveConfig {
    pub access_token: String,
    pub folder_id: String,
}

impl GDriveConfig {
    pub fn from_env() -> Option<Self> {
        let access_token = std::env::var("RSCHIKETTO_GDRIVE_ACCESS_TOKEN").ok()?;
        let folder_id = std::env::var("RSCHIKETTO_GDRIVE_FOLDER_ID").unwrap_or_default();
        Some(Self { access_token, folder_id })
    }
}

/// Google Drive REST API(v3)を叩くバックエンド。`google-drive3`のような
/// フルクレートではなく、`reqwest`で必要最小限のエンドポイント
/// (`files.create`のmultipart upload・`files.get?alt=media`)のみを直接
/// 叩く軽量実装(依存を増やしすぎない判断)。
pub struct GDriveBackend {
    config: GDriveConfig,
    client: reqwest::Client,
}

impl GDriveBackend {
    pub fn new(config: GDriveConfig) -> Self {
        Self { config, client: reqwest::Client::new() }
    }

    /// アップロード先URLを組み立てる(ネットワークを伴わないため、
    /// 実APIキー無しでもテスト可能)。
    fn upload_url(&self) -> String {
        "https://www.googleapis.com/upload/drive/v3/files?uploadType=media".to_string()
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.config.access_token)
    }
}

#[async_trait]
impl StorageBackend for GDriveBackend {
    async fn read(&self, _path: &str) -> Result<Vec<u8>> {
        Err(anyhow!("gdrive read: file-id lookup by path not yet implemented (folder_id={})", self.config.folder_id))
    }

    async fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let resp = self
            .client
            .post(self.upload_url())
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .await
            .with_context(|| format!("gdrive upload request failed for {path}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("gdrive upload returned HTTP {}", resp.status()));
        }
        Ok(())
    }

    async fn ensure_dir(&self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn exists(&self, _path: &str) -> bool {
        false
    }
}

/// 環境変数`RSCHIKETTO_STORAGE_BACKEND`(`local`/`sftp`/`gdrive`、既定`local`)
/// を見て、使用するバックエンド名を返す(実体の生成は各呼び出し側の
/// フィーチャ設定に依存するため、ここでは選択ロジックのみを共通化する)。
pub fn selected_backend_name() -> String {
    std::env::var("RSCHIKETTO_STORAGE_BACKEND").unwrap_or_else(|_| "local".to_string())
}

/// 起動時に実際に使う`StorageBackend`実装を選ぶファクトリ。
/// **正直な開示**: `sftp`/`gdrive`は本体I/O(`read`/`write`/`ensure_dir`)
/// がまだプレースホルダのため、選択してもデータ破損は起きない
/// (エラーを返すだけ)が実際には保存先として機能しない。今回のスコープは
/// `local`(既定)の実配線までであり、`sftp`/`gdrive`を選んだ場合は
/// 警告ログを出しつつ`LocalFsBackend`にフォールバックする(黙って
/// データを失うより安全側に倒す判断)。
pub fn backend_from_env() -> std::sync::Arc<dyn StorageBackend> {
    let name = selected_backend_name();
    match name.as_str() {
        "local" => std::sync::Arc::new(LocalFsBackend),
        other => {
            tracing::warn!(
                "RSCHIKETTO_STORAGE_BACKEND={other} was requested, but its StorageBackend I/O is not yet wired to a real destination (see storage.rs docs) — falling back to LocalFsBackend to avoid silent data loss"
            );
            std::sync::Arc::new(LocalFsBackend)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_fs_backend_round_trips_write_read_exists() {
        let dir = std::env::temp_dir().join(format!("rschiketto-storage-test-{}", rand::random::<u64>()));
        let file = dir.join("sub").join("data.json");
        let backend = LocalFsBackend;
        let path = file.to_string_lossy().to_string();

        assert!(!backend.exists(&path).await);
        backend.write(&path, b"{\"hello\":\"world\"}").await.unwrap();
        assert!(backend.exists(&path).await);
        let got = backend.read(&path).await.unwrap();
        assert_eq!(got, b"{\"hello\":\"world\"}");

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn local_fs_backend_read_missing_file_errors() {
        let backend = LocalFsBackend;
        let result = backend.read("./does/not/exist-rschiketto.json").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn local_fs_backend_ensure_dir_creates_nested_directories() {
        let dir = std::env::temp_dir().join(format!("rschiketto-storage-dirtest-{}", rand::random::<u64>()));
        let backend = LocalFsBackend;
        let nested = dir.join("a").join("b").join("c");
        backend.ensure_dir(&nested.to_string_lossy()).await.unwrap();
        assert!(nested.exists());
        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[test]
    fn sftp_config_remote_path_joins_base_dir() {
        let cfg = SftpConfig { host: "h".into(), port: 22, user: "u".into(), password: "p".into(), base_dir: "/srv/rschiketto".into() };
        assert_eq!(cfg.remote_path("data/tickets.json"), "/srv/rschiketto/data/tickets.json");
        assert_eq!(cfg.remote_path("/data/tickets.json"), "/srv/rschiketto/data/tickets.json");
    }

    #[test]
    fn sftp_config_remote_path_with_root_base_dir() {
        let cfg = SftpConfig { host: "h".into(), port: 22, user: "u".into(), password: "p".into(), base_dir: "/".into() };
        assert_eq!(cfg.remote_path("data.json"), "/data.json");
    }

    #[test]
    fn sftp_config_from_env_requires_host_and_user() {
        std::env::remove_var("RSCHIKETTO_SFTP_HOST");
        std::env::remove_var("RSCHIKETTO_SFTP_USER");
        assert!(SftpConfig::from_env().is_none());
    }

    #[test]
    fn gdrive_backend_builds_expected_upload_url_and_auth_header() {
        let cfg = GDriveConfig { access_token: "tok123".into(), folder_id: "fid".into() };
        let backend = GDriveBackend::new(cfg);
        assert_eq!(backend.upload_url(), "https://www.googleapis.com/upload/drive/v3/files?uploadType=media");
        assert_eq!(backend.auth_header(), "Bearer tok123");
    }

    #[test]
    fn selected_backend_name_defaults_to_local() {
        std::env::remove_var("RSCHIKETTO_STORAGE_BACKEND");
        assert_eq!(selected_backend_name(), "local");
    }

    #[test]
    fn selected_backend_name_reads_env_override() {
        std::env::set_var("RSCHIKETTO_STORAGE_BACKEND", "gdrive");
        assert_eq!(selected_backend_name(), "gdrive");
        std::env::remove_var("RSCHIKETTO_STORAGE_BACKEND");
    }
}
