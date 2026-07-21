//! 登録済みメールアドレスの管理(管理者のみが追加・削除できる)。
//! [`RGit`](https://github.com/aon-co-jp/RGit)の`src/accounts.rs`と
//! 同じ設計思想(このパスでは自己申請フローは未移植、管理者による
//! 直接登録のみ)。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountStore {
    pub emails: HashSet<String>,
}

fn accounts_path(data_root: &Path) -> PathBuf {
    data_root.join("accounts.json")
}

pub async fn load(data_root: &Path) -> AccountStore {
    match tokio::fs::read(accounts_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => AccountStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &AccountStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("AccountStore serialization is infallible");
    tokio::fs::write(accounts_path(data_root), bytes).await
}
