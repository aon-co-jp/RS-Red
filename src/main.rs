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

    let app = Route::new()
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
        .with(Tracing);

    let port = std::env::var("RSCHIKETTO_PORT").unwrap_or_else(|_| "8100".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("listening on {addr}");
    Server::new(TcpListener::bind(addr)).run(app).await?;
    Ok(())
}
