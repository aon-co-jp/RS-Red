//! チケットへのコメント(`Comment`)。RS-Blogの公開ブログコメントと違い、
//! 対象チケットが所属する`Project`への編集権限(`access::Need::Edit`相当)
//! を持つ認証済みアカウントのみが投稿できる設計のため、モデレーション
//! キュー(承認待ち)は不要——投稿時点で既に権限確認済み(`main.rs`参照)。
//! 永続化は既存の`project.rs`/`accounts.rs`と同じJSONファイルパターン。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub ticket_id: u64,
    pub author_email: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CommentStore {
    pub next_id: u64,
    pub comments: Vec<Comment>,
}

impl CommentStore {
    pub fn for_ticket(&self, ticket_id: u64) -> Vec<&Comment> {
        self.comments.iter().filter(|c| c.ticket_id == ticket_id).collect()
    }

    pub fn find(&self, id: u64) -> Option<&Comment> {
        self.comments.iter().find(|c| c.id == id)
    }
}

fn comments_path(data_root: &Path) -> PathBuf {
    data_root.join("comments.json")
}

pub async fn load(data_root: &Path) -> CommentStore {
    match tokio::fs::read(comments_path(data_root)).await {
        Ok(bytes) => crate::rustjson::parse_typed(&bytes).unwrap_or_default(),
        Err(_) => CommentStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &CommentStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("CommentStore serialization is infallible");
    tokio::fs::write(comments_path(data_root), bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_and_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rschiketto-comment-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let mut store = CommentStore::default();
        let id = store.next_id;
        store.next_id += 1;
        store.comments.push(Comment {
            id,
            ticket_id: 7,
            author_email: "member@example.com".to_string(),
            body: "looks good".to_string(),
            created_at: crate::project::now_rfc3339(),
        });
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        assert_eq!(loaded.comments.len(), 1);
        assert_eq!(loaded.for_ticket(7).len(), 1);
        assert_eq!(loaded.for_ticket(999).len(), 0);
        assert!(loaded.find(id).is_some());

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn load_missing_file_returns_default() {
        let dir = std::env::temp_dir().join(format!("rschiketto-comment-missing-{}", std::process::id()));
        let store = load(&dir).await;
        assert_eq!(store.comments.len(), 0);
        assert_eq!(store.next_id, 0);
    }
}
