//! # RS-Chiketto (v0.1.0)
//!
//! [Redmine](https://redmine.org/)(実際にはRuby on Rails製)の、
//! ハイスピード・ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)版を目指す。
//!
//! ## 正直な開示(最重要、`RGit`/`aruaru-llm`と同じ流儀)
//!
//! **v0.1.0時点では、チケット(Issue)のCRUDのみ実装している。**
//! Redmineが持つ以下の機能は**まだ一切無い**:
//!
//! - プロジェクト・サブプロジェクト階層
//! - ガントチャート・カレンダー
//! - Wiki・フォーラム
//! - リポジトリ連携(SCM閲覧、[`RGit`](https://github.com/aon-co-jp/RGit)との連携は将来検討)
//! - カスタムフィールド・ワークフロー
//!
//! 認証は[`RGit`](https://github.com/aon-co-jp/RGit)で先行実装した
//! OTPログイン(固定管理者+登録アカウント)をそのまま移植して使用。
//! ストレージは現時点でJSONファイル永続化(`aruaru-db`/PostgreSQL
//! DUAL DB構成への移行は未着手、`CLAUDE.md`のHANDOFF参照)。

mod access;
mod accounts;
mod auth;
mod mail;

use std::path::PathBuf;
use std::sync::Arc;

use poem::listener::TcpListener;
use poem::middleware::Tracing;
use poem::web::Data;
use poem::{
    get, handler, post,
    web::Path as PathExtractor,
    EndpointExt, Request, Response, Result as PoemResult, Route, Server,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct AppState {
    data_root: PathBuf,
    auth: Arc<auth::AuthStore>,
    admin_email: String,
    smtp: Option<mail::SmtpConfig>,
    /// `RSCHIKETTO_ACCOUNTS_LOCKED`(既定`true`)。`RGit`と同じ方針で、
    /// ロック中は管理者以外のアカウント登録・申請承認を拒否する。
    accounts_locked: bool,
}

fn require_admin_session(req: &Request, state: &AppState) -> PoemResult<()> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    let token = header.strip_prefix("Bearer ").unwrap_or("");
    match state.auth.session_email(token) {
        Some(email) if email == state.admin_email => Ok(()),
        _ => Err(poem::Error::from_string("admin login required", poem::http::StatusCode::UNAUTHORIZED)),
    }
}

/// リクエストの`Authorization: Bearer`ヘッダからログイン中のメール
/// アドレスを取得する(未ログインなら`None`、管理者・一般アカウント
/// いずれも区別しない)。
fn session_email(req: &Request, state: &AppState) -> Option<String> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    let token = header.strip_prefix("Bearer ").unwrap_or("");
    state.auth.session_email(token)
}

/// チケットが所属する`project`に対して`need`の操作が許可されているかを
/// 判定する(`access.rs`の`is_allowed`を利用)。管理者は常に許可。
/// 未ログインは`401`、ログイン済みだが権限不足は`403`
/// (`RGit`と同じ401/403の使い分け)。
async fn check_project_access(req: &Request, state: &AppState, project: &str, need: access::Need) -> PoemResult<()> {
    let email = session_email(req, state);
    if let Some(email) = &email {
        if *email == state.admin_email {
            return Ok(());
        }
    }
    let config = access::load(&state.data_root, project_id(project)).await;
    if access::is_allowed(&config, need, email.as_deref()) {
        return Ok(());
    }
    if email.is_none() {
        Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED))
    } else {
        Err(poem::Error::from_string("insufficient permission", poem::http::StatusCode::FORBIDDEN))
    }
}

/// プロジェクト名(文字列)から`access.rs`が使う`project_id`(u64)を
/// 導出する(v0.1.0時点ではプロジェクトはCRUDを持たない単純な文字列
/// ラベルのため、アクセス設定ファイル名の一意性はハッシュ値に委ねる)。
fn project_id(project: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    project.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TicketStatus {
    Open,
    InProgress,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ticket {
    id: u64,
    title: String,
    description: String,
    status: TicketStatus,
    /// チケットが所属するプロジェクト名(単純な文字列ラベル、v0.1.0
    /// 時点ではProject自体のCRUDは無い——`access.rs`のアクセス制御に
    /// 何かグループ単位を与えるための最小実装、CLAUDE.md参照)。
    /// 空文字列は「未分類」を表し、`RSCHIKETTO_ADMIN_EMAIL`以外は
    /// private既定によりデフォルトで閲覧不可。
    #[serde(default)]
    project: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TicketStore {
    next_id: u64,
    tickets: Vec<Ticket>,
}

fn tickets_path(data_root: &std::path::Path) -> PathBuf {
    data_root.join("tickets.json")
}

async fn load_tickets(data_root: &std::path::Path) -> TicketStore {
    match tokio::fs::read(tickets_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => TicketStore::default(),
    }
}

async fn save_tickets(data_root: &std::path::Path, store: &TicketStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("TicketStore serialization is infallible");
    tokio::fs::write(tickets_path(data_root), bytes).await
}

#[derive(Deserialize)]
struct CreateTicketRequest {
    title: String,
    description: String,
    /// 所属プロジェクト(空文字列可、既定は「未分類」——`access.rs`の
    /// `Mode::Private`既定により管理者以外は不可視)。
    #[serde(default)]
    project: String,
}

/// `POST /api/tickets` — チケットを新規作成する。所属`project`への
/// `Need::Edit`権限が必要(管理者は常に許可、`access.rs`参照)。
#[handler]
async fn create_ticket(req: &Request, state: Data<&AppState>, body: poem::web::Json<CreateTicketRequest>) -> PoemResult<Response> {
    check_project_access(req, &state, &body.project, access::Need::Edit).await?;
    if body.title.trim().is_empty() {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("title must not be empty"));
    }
    let mut store = load_tickets(&state.data_root).await;
    let id = store.next_id;
    store.next_id += 1;
    let ticket =
        Ticket { id, title: body.title.clone(), description: body.description.clone(), status: TicketStatus::Open, project: body.project.clone() };
    store.tickets.push(ticket.clone());
    save_tickets(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder()
        .status(poem::http::StatusCode::CREATED)
        .content_type("application/json")
        .body(serde_json::to_vec(&ticket).unwrap_or_default()))
}

/// `GET /api/tickets` — チケット一覧。各チケットは所属`project`への
/// `Need::View`権限がある場合のみ結果に含める(管理者は全件、
/// 未ログインは基本的に空配列——`RGit`と同じprivate既定の考え方)。
#[handler]
async fn list_tickets(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let email = session_email(req, &state);
    let is_admin = email.as_deref() == Some(state.admin_email.as_str());
    let store = load_tickets(&state.data_root).await;
    let mut visible = Vec::new();
    for ticket in &store.tickets {
        if is_admin {
            visible.push(ticket.clone());
            continue;
        }
        let config = access::load(&state.data_root, project_id(&ticket.project)).await;
        if access::is_allowed(&config, access::Need::View, email.as_deref()) {
            visible.push(ticket.clone());
        }
    }
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&visible).unwrap_or_default()))
}

#[handler]
async fn get_ticket(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let store = load_tickets(&state.data_root).await;
    match store.tickets.iter().find(|t| t.id == id) {
        Some(ticket) => {
            check_project_access(req, &state, &ticket.project, access::Need::View).await?;
            Ok(Response::builder()
                .status(poem::http::StatusCode::OK)
                .content_type("application/json")
                .body(serde_json::to_vec(ticket).unwrap_or_default()))
        }
        None => Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("ticket not found")),
    }
}

#[derive(Deserialize)]
struct UpdateTicketRequest {
    title: Option<String>,
    description: Option<String>,
    status: Option<TicketStatus>,
}

/// `PUT /api/tickets/:id` — チケットのタイトル・説明・ステータスを更新する
/// (所属`project`への`Need::Edit`権限が必要、指定したフィールドのみ更新)。
#[handler]
async fn update_ticket(
    req: &Request,
    PathExtractor(id): PathExtractor<u64>,
    state: Data<&AppState>,
    body: poem::web::Json<UpdateTicketRequest>,
) -> PoemResult<Response> {
    let store_preview = load_tickets(&state.data_root).await;
    let Some(existing) = store_preview.tickets.iter().find(|t| t.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("ticket not found"));
    };
    check_project_access(req, &state, &existing.project, access::Need::Edit).await?;
    let mut store = store_preview;
    let Some(ticket) = store.tickets.iter_mut().find(|t| t.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("ticket not found"));
    };
    if let Some(title) = &body.title {
        ticket.title = title.clone();
    }
    if let Some(description) = &body.description {
        ticket.description = description.clone();
    }
    if let Some(status) = &body.status {
        ticket.status = status.clone();
    }
    let updated = ticket.clone();
    save_tickets(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&updated).unwrap_or_default()))
}

/// トップページ(`GET /`)のHTMLランディングページ。
/// ブラウザで実インスタンスへアクセスしたユーザーへ、アプリの概要・
/// 実装済みAPI一覧・未実装機能の正直な開示・ダウンロードリンクを示す
/// (JSON APIのみで何も表示されないUXバグの修正、`RGit`の
/// `static/index.html`と同じ趣旨)。
const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>RS-Chiketto</title>
<style>
  body { font-family: system-ui, sans-serif; max-width: 780px; margin: 2rem auto; padding: 0 1rem; line-height: 1.6; color: #222; }
  h1 { margin-bottom: 0; }
  .tagline { color: #666; margin-top: 0.2rem; }
  code { background: #f2f2f2; padding: 0.1rem 0.35rem; border-radius: 3px; }
  table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
  th, td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #ddd; font-size: 0.92rem; }
  .warn { background: #fff8e1; border: 1px solid #ffe08a; border-radius: 6px; padding: 0.8rem 1rem; }
  .btn { display: inline-block; background: #2d6cdf; color: #fff; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; margin-right: 0.5rem; }
  footer { color: #888; font-size: 0.85rem; margin-top: 2rem; }
</style>
</head>
<body>
<h1>RS-Chiketto</h1>
<p class="tagline">Redmine相当のチケット(Issue)トラッカー — Rust + poem(RPoem)製、高速・高セキュリティ・省メモリ志向。v0.1.0。</p>

<h2>これは何?</h2>
<p>
  <a href="https://redmine.org/">Redmine</a>のRust版を目指すプロジェクトです。
  v0.1.0時点ではチケット管理とOTPログイン・アクセス制御のみを実装しています。
</p>

<h2>使い方: 現在はJSON APIのみ(ブラウザUIはまだありません)</h2>
<p>このページ以外はすべてJSON APIです。以下のエンドポイントに対して<code>curl</code>や外部クライアントからアクセスしてください。</p>
<table>
<tr><th>メソッド / パス</th><th>説明</th></tr>
<tr><td><code>GET /healthz</code></td><td>ヘルスチェック</td></tr>
<tr><td><code>POST /api/auth/request-otp</code></td><td>ログイン用ワンタイムパスワードをメール送信</td></tr>
<tr><td><code>POST /api/auth/verify-otp</code></td><td>OTPを検証してセッショントークンを発行</td></tr>
<tr><td><code>POST /api/auth/logout</code></td><td>ログアウト(トークン失効)</td></tr>
<tr><td><code>GET /api/accounts</code> / <code>POST /api/accounts</code></td><td>登録アカウント一覧取得 / 追加(管理者のみ)</td></tr>
<tr><td><code>POST /api/accounts/request</code></td><td>アカウント利用の自己申請(認証不要)</td></tr>
<tr><td><code>GET /api/accounts/requests</code></td><td>保留中の自己申請一覧(管理者のみ)</td></tr>
<tr><td><code>POST /api/accounts/requests/:id/decide</code></td><td>自己申請の承認/却下・プロジェクトへの閲覧/編集権限付与(管理者のみ)</td></tr>
<tr><td><code>GET /api/tickets</code> / <code>POST /api/tickets</code></td><td>チケット一覧取得(アクセス権のあるプロジェクトのみ) / 新規作成</td></tr>
<tr><td><code>GET /api/tickets/:id</code> / <code>PUT /api/tickets/:id</code></td><td>チケット詳細取得 / 更新(ステータス変更含む)</td></tr>
</table>

<div class="warn">
<strong>正直な開示: まだ実装していない機能</strong>
<ul>
<li>プロジェクト・サブプロジェクト階層(現状はチケットに文字列ラベルを付与するのみ)</li>
<li>ガントチャート・カレンダー</li>
<li>Wiki・フォーラム</li>
<li>リポジトリ連携(SCM閲覧、<a href="https://github.com/aon-co-jp/RGit">RGit</a>との連携は将来検討)</li>
<li>カスタムフィールド・ワークフロー</li>
<li><code>aruaru-db</code>/PostgreSQLへの移行(現状はJSONファイル永続化)</li>
</ul>
</div>

<h2>ダウンロード / インストール</h2>
<p>
  <a class="btn" href="https://github.com/aon-co-jp/RS-Chiketto/releases/latest">最新リリースをダウンロード</a>
  <a class="btn" href="https://github.com/aon-co-jp/RS-Chiketto">GitHubでソースを見る</a>
</p>
<p>Linux(静的リンクmuslバイナリ)・Windows向けにインストーラー付きビルド済みバイナリを配布しています。詳細は<a href="https://github.com/aon-co-jp/RS-Chiketto#readme">README</a>参照。</p>

<footer>RS-Chiketto v0.1.0 &mdash; <a href="https://github.com/aon-co-jp/RS-Chiketto">aon-co-jp/RS-Chiketto</a></footer>
</body>
</html>
"#;

#[handler]
async fn index() -> Response {
    Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(INDEX_HTML)
}

#[handler]
async fn healthz() -> &'static str {
    "ok"
}

#[handler]
async fn request_otp(state: Data<&AppState>, body: poem::web::Json<serde_json::Value>) -> PoemResult<Response> {
    let email = body.get("email").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if email != state.admin_email {
        let registered = accounts::load(&state.data_root).await;
        if !registered.emails.contains(&email) {
            return Ok(Response::builder().status(poem::http::StatusCode::FORBIDDEN).body("email not registered"));
        }
    }
    let Some(smtp) = state.smtp.clone() else {
        return Ok(Response::builder().status(poem::http::StatusCode::SERVICE_UNAVAILABLE).body("SMTP not configured"));
    };
    let auth::RequestOtpOutcome::Issued(code) = state.auth.request_otp(&email);
    match mail::send_otp(smtp, email, code).await {
        Ok(()) => Ok(Response::builder().status(poem::http::StatusCode::OK).body("otp sent")),
        Err(e) => {
            tracing::warn!("failed to send OTP mail: {e}");
            Ok(Response::builder().status(poem::http::StatusCode::BAD_GATEWAY).body("failed to send mail"))
        }
    }
}

#[derive(Deserialize)]
struct VerifyOtpRequest {
    email: String,
    code: String,
}

#[handler]
async fn verify_otp(state: Data<&AppState>, body: poem::web::Json<VerifyOtpRequest>) -> PoemResult<Response> {
    match state.auth.consume_otp(&body.email, &body.code) {
        Ok(()) => {
            let token = state.auth.create_session(&body.email);
            Ok(Response::builder()
                .status(poem::http::StatusCode::OK)
                .content_type("application/json")
                .body(serde_json::to_vec(&serde_json::json!({ "token": token })).unwrap_or_default()))
        }
        Err(e) => Ok(Response::builder().status(poem::http::StatusCode::FORBIDDEN).body(e.message())),
    }
}

/// `POST /api/auth/logout` — セッショントークンを失効させる。
#[handler]
async fn logout(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    if let Some(token) = header.strip_prefix("Bearer ") {
        state.auth.logout(token);
    }
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("logged out"))
}

#[derive(Deserialize)]
struct AddAccountRequest {
    email: String,
}

/// `POST /api/accounts` — ログイン可能なメールアドレスを1件登録する
/// (管理者のみ)。`accounts_locked`中は管理者メール以外を拒否する。
#[handler]
async fn add_account(req: &Request, state: Data<&AppState>, body: poem::web::Json<AddAccountRequest>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let email = body.email.trim().to_string();
    if !email.contains('@') {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("invalid email"));
    }
    if state.accounts_locked && email != state.admin_email {
        return Ok(Response::builder()
            .status(poem::http::StatusCode::FORBIDDEN)
            .body("account registration is currently restricted to the administrator email only"));
    }
    let mut store = accounts::load(&state.data_root).await;
    store.emails.insert(email);
    accounts::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::CREATED).body("ok"))
}

/// `GET /api/accounts` — 登録済みメールアドレス一覧(管理者のみ)。
#[handler]
async fn list_accounts(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let store = accounts::load(&state.data_root).await;
    let mut emails: Vec<&String> = store.emails.iter().collect();
    emails.sort();
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&emails).unwrap_or_default()))
}

#[derive(Deserialize)]
struct AccessRequestPayload {
    email: String,
    #[serde(default)]
    message: Option<String>,
}

/// `POST /api/accounts/request` — **認証不要、誰でも申請可能**。
/// ログイン許可を求める申請を保留リストへ追加する
/// (管理者が[`decide_access_request`]で許可するまでは無効)。
#[handler]
async fn request_access(state: Data<&AppState>, body: poem::web::Json<AccessRequestPayload>) -> PoemResult<Response> {
    let email = body.email.trim().to_string();
    if !email.contains('@') {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("invalid email"));
    }
    let mut store = accounts::load(&state.data_root).await;
    let id = accounts::generate_request_id();
    store.pending_requests.push(accounts::AccessRequest { id, email: email.clone(), message: body.message.clone() });
    accounts::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    if let Some(smtp) = state.smtp.clone() {
        if let Err(e) = mail::send_access_request_notice(smtp, state.admin_email.clone(), email, body.message.clone()).await {
            tracing::warn!("failed to notify admin of access request: {e}");
        }
    }
    Ok(Response::builder().status(poem::http::StatusCode::CREATED).body("request submitted"))
}

/// `GET /api/accounts/requests` — 保留中の申請一覧(管理者のみ)。
#[handler]
async fn list_access_requests(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let store = accounts::load(&state.data_root).await;
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&store.pending_requests).unwrap_or_default()))
}

#[derive(Deserialize)]
struct DecideAccessRequestPayload {
    approve: bool,
    #[serde(default)]
    allow_view: bool,
    #[serde(default)]
    allow_edit: bool,
    #[serde(default)]
    project: Option<String>,
}

/// `POST /api/accounts/requests/:id/decide` — 申請を審査する(管理者のみ)。
/// 承認時、`project`が指定されていればそのプロジェクトの
/// `access::AccessConfig::accounts`に閲覧/編集許可を書き込む
/// (プロジェクト指定が無い申請はアカウント登録のみ行う)。
/// `accounts_locked`中は管理者メール以外の承認を拒否する
/// (`RGit`の`RGIT_ACCOUNTS_LOCKED`と同じ方針)。
#[handler]
async fn decide_access_request(
    req: &Request,
    PathExtractor(id): PathExtractor<String>,
    state: Data<&AppState>,
    body: poem::web::Json<DecideAccessRequestPayload>,
) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let mut store = accounts::load(&state.data_root).await;
    let Some(pos) = store.pending_requests.iter().position(|r| r.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("request not found"));
    };
    let request = store.pending_requests.remove(pos);

    if body.approve && state.accounts_locked && request.email != state.admin_email {
        accounts::save(&state.data_root, &store)
            .await
            .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
        return Ok(Response::builder()
            .status(poem::http::StatusCode::FORBIDDEN)
            .body("account registration is currently restricted to the administrator email only"));
    }

    if body.approve {
        store.emails.insert(request.email.clone());
    }
    accounts::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;

    if body.approve {
        if let Some(project) = &body.project {
            let mut config = access::load(&state.data_root, project_id(project)).await;
            config.accounts.insert(request.email.clone(), access::AccountPermission { allow_view: body.allow_view, allow_edit: body.allow_edit });
            access::save(&state.data_root, project_id(project), &config)
                .await
                .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
        }
    }

    if let Some(smtp) = state.smtp.clone() {
        if let Err(e) = mail::send_access_decision(smtp, request.email.clone(), body.approve).await {
            tracing::warn!("failed to notify requester of decision: {e}");
        }
    }
    Ok(Response::builder().status(poem::http::StatusCode::OK).body(if body.approve { "approved" } else { "denied" }))
}

fn env_data_dir() -> PathBuf {
    std::env::var("RSCHIKETTO_DATA_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("./data"))
}

/// ルーティング定義を`main()`とテスト(`poem::test::TestClient`)の両方から
/// 再利用できるように切り出したもの。
fn build_routes(state: AppState) -> impl poem::Endpoint {
    Route::new()
        .at("/", get(index))
        .at("/healthz", get(healthz))
        .at("/api/auth/request-otp", post(request_otp))
        .at("/api/auth/verify-otp", post(verify_otp))
        .at("/api/auth/logout", post(logout))
        .at("/api/accounts", get(list_accounts).post(add_account))
        .at("/api/accounts/request", post(request_access))
        .at("/api/accounts/requests", get(list_access_requests))
        .at("/api/accounts/requests/:id/decide", post(decide_access_request))
        .at("/api/tickets", get(list_tickets).post(create_ticket))
        .at("/api/tickets/:id", get(get_ticket).put(update_ticket))
        .data(state)
        .with(Tracing)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let data_root = env_data_dir();
    tokio::fs::create_dir_all(&data_root).await?;
    tracing::info!("rs-chiketto v0.1.0 starting, data_root={:?}", data_root);

    let admin_email = std::env::var("RSCHIKETTO_ADMIN_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
    let smtp = mail::SmtpConfig::from_env();
    if smtp.is_none() {
        tracing::warn!("RSCHIKETTO_SMTP_* not fully configured; /api/auth/request-otp will return 503");
    }
    let accounts_locked = std::env::var("RSCHIKETTO_ACCOUNTS_LOCKED").map(|v| v != "false" && v != "0").unwrap_or(true);
    if accounts_locked {
        tracing::info!("account registration is locked to the admin email only (RSCHIKETTO_ACCOUNTS_LOCKED=false to lift)");
    }
    let state = AppState { data_root, auth: Arc::new(auth::AuthStore::default()), admin_email, smtp, accounts_locked };

    let app = build_routes(state);

    let port = std::env::var("RSCHIKETTO_PORT").unwrap_or_else(|_| "8100".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("listening on {addr}");
    Server::new(TcpListener::bind(addr)).run(app).await?;
    Ok(())
}

#[cfg(test)]
mod handler_tests {
    //! `poem::test::TestClient`を使ったハンドラレベルの統合テスト
    //! (2026-07-21追記、HANDOFF記載の宿題への対応)。
    //! `cargo test`実行時にテストごとに独立した一時ディレクトリを
    //! `RSCHIKETTO_DATA_DIR`として使うため、`AppState.data_root`は各テスト
    //! ごとに直接構築する(実プロセスの環境変数には依存しない)。

    use super::*;
    use poem::test::TestClient;

    const ADMIN_EMAIL: &str = "admin@example.com";

    fn temp_dir(label: &str) -> PathBuf {
        let unique = accounts::generate_request_id();
        std::env::temp_dir().join(format!("rschiketto-handler-test-{label}-{unique}"))
    }

    /// `accounts_locked`を指定してテスト用の`AppState`を構築する
    /// (環境変数に依存しないテストローカル構築、SMTP未設定)。
    async fn make_state(label: &str, accounts_locked: bool) -> AppState {
        let data_root = temp_dir(label);
        tokio::fs::create_dir_all(&data_root).await.unwrap();
        AppState { data_root, auth: Arc::new(auth::AuthStore::default()), admin_email: ADMIN_EMAIL.to_string(), smtp: None, accounts_locked }
    }

    /// 管理者としてログイン済みのセッショントークンを、OTPフローを経由
    /// せず直接`AuthStore`に発行させて得る(SMTP無し環境でもテスト可能)。
    fn admin_token(state: &AppState) -> String {
        state.auth.create_session(ADMIN_EMAIL)
    }

    #[tokio::test]
    async fn unauthenticated_list_tickets_returns_200_with_empty_array() {
        // HANDOFFに明記された設計: 未ログインの`GET /api/tickets`は
        // 401ではなく200・空配列(project単位のフィルタリングにより
        // 可視チケットが0件になるため)。
        let state = make_state("list-empty", true).await;
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client.get("/api/tickets").send().await;
        resp.assert_status_is_ok();
        resp.assert_text("[]").await;
    }

    #[tokio::test]
    async fn root_returns_landing_page_with_key_markers() {
        // UXバグ修正の検証: JSON APIオンリーで何も表示されなかった`GET /`が
        // アプリ名・実エンドポイント・ダウンロードリンクを含むHTMLを返すこと。
        let state = make_state("landing-page", true).await;
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client.get("/").send().await;
        resp.assert_status_is_ok();
        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(body.contains("RS-Chiketto"));
        assert!(body.contains("/api/tickets"));
        assert!(body.contains("https://github.com/aon-co-jp/RS-Chiketto/releases/latest"));
    }

    #[tokio::test]
    async fn self_service_account_request_returns_201_and_creates_pending_request() {
        let state = make_state("self-service-request", true).await;
        let data_root = state.data_root.clone();
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client
            .post("/api/accounts/request")
            .body_json(&serde_json::json!({ "email": "newcomer@example.com", "message": "please let me in" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);

        let store = accounts::load(&data_root).await;
        assert_eq!(store.pending_requests.len(), 1);
        assert_eq!(store.pending_requests[0].email, "newcomer@example.com");
    }

    #[tokio::test]
    async fn admin_approving_a_request_grants_the_expected_access_config_entry() {
        let state = make_state("approve-grants-access", false).await;
        let data_root = state.data_root.clone();
        let token = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);

        // まず自己申請を作成。
        client
            .post("/api/accounts/request")
            .body_json(&serde_json::json!({ "email": "member@example.com" }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        let store = accounts::load(&data_root).await;
        let request_id = store.pending_requests[0].id.clone();

        // 管理者セッションで承認、プロジェクト"demo"へview権限を付与。
        let resp = client
            .post(format!("/api/accounts/requests/{request_id}/decide"))
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({
                "approve": true,
                "allow_view": true,
                "allow_edit": false,
                "project": "demo"
            }))
            .send()
            .await;
        resp.assert_status_is_ok();

        // 承認によりaccounts一覧へ追加されていること。
        let updated_store = accounts::load(&data_root).await;
        assert!(updated_store.emails.contains("member@example.com"));
        assert!(updated_store.pending_requests.is_empty());

        // access::AccessConfigへ期待した許可が書き込まれていること。
        let config = access::load(&data_root, project_id("demo")).await;
        let perm = config.accounts.get("member@example.com").expect("member should have an access grant");
        assert!(perm.allow_view);
        assert!(!perm.allow_edit);
    }

    #[tokio::test]
    async fn accounts_locked_rejects_non_admin_approval_with_403() {
        // このテストはローカル構築の`AppState`で`accounts_locked: true`を
        // 指定するのみで、プロセス環境変数`RSCHIKETTO_ACCOUNTS_LOCKED`は
        // 一切変更しない(他テストへの影響を避けるため)。
        let state = make_state("locked-rejects-approval", true).await;
        let data_root = state.data_root.clone();
        let token = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);

        client
            .post("/api/accounts/request")
            .body_json(&serde_json::json!({ "email": "outsider@example.com" }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        let store = accounts::load(&data_root).await;
        let request_id = store.pending_requests[0].id.clone();

        // 管理者セッションであっても、承認対象が管理者メール以外かつ
        // accounts_locked中は403で拒否される(main.rsのdecide_access_request参照)。
        let resp = client
            .post(format!("/api/accounts/requests/{request_id}/decide"))
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({ "approve": true }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::FORBIDDEN);

        // 拒否されても申請自体はpendingリストから取り除かれ、emailsには
        // 追加されていないこと(main.rsの実装通り)。
        let after = accounts::load(&data_root).await;
        assert!(!after.emails.contains("outsider@example.com"));
    }
}
