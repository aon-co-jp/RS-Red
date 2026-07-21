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
}

fn require_admin_session(req: &Request, state: &AppState) -> PoemResult<()> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    let token = header.strip_prefix("Bearer ").unwrap_or("");
    match state.auth.session_email(token) {
        Some(email) if email == state.admin_email => Ok(()),
        _ => Err(poem::Error::from_string("admin login required", poem::http::StatusCode::UNAUTHORIZED)),
    }
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
}

/// `POST /api/tickets` — チケットを新規作成する(ログイン必須)。
/// v0.1.0時点ではプロジェクト分割・担当者割り当ては未実装、全チケットが
/// 単一のフラットな一覧に入る(次の増分で対応、CLAUDE.md参照)。
#[handler]
async fn create_ticket(req: &Request, state: Data<&AppState>, body: poem::web::Json<CreateTicketRequest>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    if body.title.trim().is_empty() {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("title must not be empty"));
    }
    let mut store = load_tickets(&state.data_root).await;
    let id = store.next_id;
    store.next_id += 1;
    let ticket = Ticket { id, title: body.title.clone(), description: body.description.clone(), status: TicketStatus::Open };
    store.tickets.push(ticket.clone());
    save_tickets(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder()
        .status(poem::http::StatusCode::CREATED)
        .content_type("application/json")
        .body(serde_json::to_vec(&ticket).unwrap_or_default()))
}

/// `GET /api/tickets` — チケット一覧(ログイン必須、v0.1.0時点では
/// 閲覧範囲の絞り込みは無く管理者のみが閲覧可能——`RGit`のような
/// アクセス制御の細分化は次の増分で追加する)。
#[handler]
async fn list_tickets(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let store = load_tickets(&state.data_root).await;
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&store.tickets).unwrap_or_default()))
}

#[handler]
async fn get_ticket(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let store = load_tickets(&state.data_root).await;
    match store.tickets.iter().find(|t| t.id == id) {
        Some(ticket) => Ok(Response::builder()
            .status(poem::http::StatusCode::OK)
            .content_type("application/json")
            .body(serde_json::to_vec(ticket).unwrap_or_default())),
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
/// (ログイン必須、指定したフィールドのみ更新)。
#[handler]
async fn update_ticket(
    req: &Request,
    PathExtractor(id): PathExtractor<u64>,
    state: Data<&AppState>,
    body: poem::web::Json<UpdateTicketRequest>,
) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let mut store = load_tickets(&state.data_root).await;
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
        return Ok(Response::builder().status(poem::http::StatusCode::FORBIDDEN).body("email not registered"));
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
    let state = AppState { data_root, auth: Arc::new(auth::AuthStore::default()), admin_email, smtp };

    let app = Route::new()
        .at("/healthz", get(healthz))
        .at("/api/auth/request-otp", post(request_otp))
        .at("/api/auth/verify-otp", post(verify_otp))
        .at("/api/auth/logout", post(logout))
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
