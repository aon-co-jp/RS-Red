//! プロジェクトごとのWikiページ(`WikiPage`)。Redmineのプロジェクト単位
//! Wikiに相当。閲覧/編集権限は所属`Project`への`access.rs`権限
//! (閲覧=Need::View、編集=Need::Edit)をそのまま再利用する(既存の
//! `comments.rs`と同じ権限モデル)。ページ名(`slug`)はプロジェクト内で
//! 一意。編集のたびに新しい`WikiRevision`を追記し、旧内容は保持する
//! (Redmineのページ履歴に相当する最小実装、差分表示は今回スコープ外)。
//! 永続化は既存モジュールと同じJSONファイルパターン。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiRevision {
    pub body: String,
    pub author_email: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    pub id: u64,
    pub project_id: u64,
    /// URLで使う識別子(例: `getting-started`)。同一プロジェクト内で一意。
    pub slug: String,
    pub title: String,
    pub revisions: Vec<WikiRevision>,
}

impl WikiPage {
    pub fn latest(&self) -> Option<&WikiRevision> {
        self.revisions.last()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WikiStore {
    pub next_id: u64,
    pub pages: Vec<WikiPage>,
}

impl WikiStore {
    pub fn for_project(&self, project_id: u64) -> Vec<&WikiPage> {
        self.pages.iter().filter(|p| p.project_id == project_id).collect()
    }

    pub fn find(&self, id: u64) -> Option<&WikiPage> {
        self.pages.iter().find(|p| p.id == id)
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut WikiPage> {
        self.pages.iter_mut().find(|p| p.id == id)
    }

    pub fn find_by_slug(&self, project_id: u64, slug: &str) -> Option<&WikiPage> {
        self.pages.iter().find(|p| p.project_id == project_id && p.slug == slug)
    }

    /// 同一プロジェクト内で`slug`が既に使われているか(新規作成時の重複防止)。
    pub fn slug_taken(&self, project_id: u64, slug: &str) -> bool {
        self.find_by_slug(project_id, slug).is_some()
    }
}

fn wiki_path(data_root: &Path) -> PathBuf {
    data_root.join("wiki.json")
}

pub async fn load(data_root: &Path) -> WikiStore {
    match tokio::fs::read(wiki_path(data_root)).await {
        Ok(bytes) => crate::rustjson::parse_typed(&bytes).unwrap_or_default(),
        Err(_) => WikiStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &WikiStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("WikiStore serialization is infallible");
    tokio::fs::write(wiki_path(data_root), bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revision(body: &str) -> WikiRevision {
        WikiRevision { body: body.to_string(), author_email: "member@example.com".to_string(), created_at: crate::project::now_rfc3339() }
    }

    #[tokio::test]
    async fn save_and_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rschiketto-wiki-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let mut store = WikiStore::default();
        let id = store.next_id;
        store.next_id += 1;
        store.pages.push(WikiPage {
            id,
            project_id: 3,
            slug: "getting-started".to_string(),
            title: "Getting Started".to_string(),
            revisions: vec![revision("hello")],
        });
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        assert_eq!(loaded.pages.len(), 1);
        assert_eq!(loaded.for_project(3).len(), 1);
        assert_eq!(loaded.for_project(999).len(), 0);
        assert!(loaded.find(id).is_some());
        assert_eq!(loaded.find_by_slug(3, "getting-started").unwrap().id, id);
        assert!(loaded.slug_taken(3, "getting-started"));
        assert!(!loaded.slug_taken(3, "other-page"));

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[test]
    fn latest_returns_most_recent_revision() {
        let page = WikiPage {
            id: 1,
            project_id: 1,
            slug: "x".to_string(),
            title: "X".to_string(),
            revisions: vec![revision("v1"), revision("v2")],
        };
        assert_eq!(page.latest().unwrap().body, "v2");
    }

    #[test]
    fn latest_of_empty_revisions_is_none() {
        let page = WikiPage { id: 1, project_id: 1, slug: "x".to_string(), title: "X".to_string(), revisions: vec![] };
        assert!(page.latest().is_none());
    }

    #[tokio::test]
    async fn load_missing_file_returns_default() {
        let dir = std::env::temp_dir().join(format!("rschiketto-wiki-missing-{}", std::process::id()));
        let store = load(&dir).await;
        assert_eq!(store.pages.len(), 0);
        assert_eq!(store.next_id, 0);
    }
}
