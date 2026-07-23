# 開発方針＆開発環境ルール(RS-Red)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RS-Red](https://github.com/aon-co-jp/RS-Red)
(旧名`RS-Chiketto`、2026-07-22に`RS-Red`へ改名)。
VPS上の作業パス: `/root/RS-Red`(2026-07-22改名、旧`/root/RS-Chiketto`から移動)。
公開先: `https://runo.tokyo/RS-Red`(旧`/chiketto`は301リダイレクト)。

## このプロジェクトの役割

[Redmine](https://redmine.org/)(実際にはRuby on Rails製)の、
ハイスピード・ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)
(RPoem)版を目指す。`RS-Git`(Gitea相当)・`RJSON`(JSON処理)と同じ
`aon-co-jp`エコシステムの一員。

> ⚠️ **正直な開示**: 2026-07-21時点でコード未着手(このCLAUDE.mdのみの
> 状態)。このエコシステム共通の方針として、実装が追いつくまでは
> 「Redmineの代替品」を名乗らず、進捗をこのHANDOFFに正直に記録する。

## 着手時に踏襲すべき既存プロジェクトの設計方針

- **`RS-Git`**(git smart HTTP・OTPログイン・アクセス制御・容量ベースの
  自動判定)を先行実装として参照。特に「正直な開示」「段階的実装」
  「型チェックだけで完了と報告しない・実機検証必須」の3方針は共通。
- **`RJSON`**(依存を絞った設計、`light`/`full`のfeature分離)も
  参照——RS-Chiketto側で構造化データ処理が必要になった際の候補。

## Redmineの主要機能(着手時の優先順位付けの参考)

- チケット(Issue)管理・ワークフロー・カスタムフィールド
- プロジェクト・サブプロジェクト階層
- ガントチャート・カレンダー
- Wiki・フォーラム
- リポジトリ連携(SCM閲覧)
- ユーザー・ロール・権限管理

## 方針決定事項(2026-07-21、ユーザー確認済み)

- **着手順番**: `RS-Chiketto`・`RS-Blog`・`RS-EC`は同時並行ではなく
  **1つずつ順番に、`RS-Git`と同じ深さまで作り込んでから次へ**進める。
  どれを最初にするかは次回セッション冒頭で決定。
- **データベース**: `aruaru-db`(ZFS互換・ACID互換のRust製DB、
  `open-raid-z`エコシステム)を採用し、3プロジェクトで統一する。
  加えて**PostgreSQLとのDUAL DATABASE構成も可能にする**(ユーザー指示、
  2026-07-21追記)——`open-runo`/RPoemが既に採用している「4層4重」の
  DUAL DB思想と同じ方針。`aruaru-db`単独運用とDUAL構成のどちらで動くかを
  設定で切り替えられる設計とし、片方に依存しすぎないアーキテクチャに
  する(実装時、`open-runo`/RPoem側のDUAL DB実装を先行事例として参照)。
- **「分身の術」構成でDB層を共有する**(ユーザー指示、2026-07-21追記):
  `open-web-server`・`aruaru-llm`・RPoem/RCosmo・`open-web-server`と
  同じ設計思想により、`aruaru-db`/PostgreSQL接続は**1インスタンスを
  複数ドメイン(RS-Chiketto自身も含め、将来の`RS-Blog`/`RS-EC`他)が
  共有**する。ドメイン・プロジェクトを追加するたびに個別にDBを
  インストール・起動する必要はない。実装時は`aruaru-llm`の
  `src/tenants.rs`(`TenantRegistry`、`RwLock`によるプロセス内共有状態、
  再起動不要で実行時追加・削除可能)と同じパターンを踏襲する。
  **管理は`open-easy-web`側から行う**(ユーザー指示、2026-07-21追記)
  ——`aruaru-llm`が`open-easy-web/server/src/appserver_registration.rs`の
  `AppServerKind::AruaruLlm`/`register_aruaru_llm()`経由でテナント登録
  される設計と同じパターンで、`RS-Chiketto`(および将来の`RS-Blog`/
  `RS-EC`)用の`AppServerKind`variantを追加し、ドメイン追加を
  `open-easy-web`の「サイト管理」画面から一元管理できるようにする。
  **非同期・マルチCPU/マルチコア/マルチスレッド対応**:
  `#[tokio::main]`は既定の`multi_thread`フレーバー(`current_thread`への
  固定はしない)、CPU負荷の高い処理は`rayon`で全論理コアへ並列
  ディスパッチする(`aruaru-llm`の`opencuda_cpu::CpuDevice`と同じ方針)。

## 公開先・配布方針(2026-07-21、ユーザー確認済み)

- **公開パス**: `runo.tokyo/chiketto`(`RS-Git`の`runo.tokyo/rgit`と同じ
  パス方式、VPS上のポートは`8100`)。
- **クロスプラットフォーム配布**: AlmaLinux・Ubuntu・Debian・Fedora・
  RHEL等の主要Linuxディストリ、Windows・Windows Server向けに、
  インストーラー付きのビルド済みバイナリをGitHub Releasesで配布する
  (ユーザー指示)。`.github/workflows/release.yml`でタグpush時に
  自動ビルド、Linux版は`x86_64-unknown-linux-musl`(静的リンク、
  ディストリ非依存)、Windows版は`x86_64-pc-windows-msvc`。
  `install.sh`(systemdサービス登録)・`install.ps1`(Windowsサービス
  登録手順の案内)を同梱。詳細は`README.md`参照。

## HANDOFF

- **2026-07-23 プロジェクト単位Wikiを追加(HANDOFF記載の宿題「Wiki・
  ガントチャート等の追加機能」への対応、ユーザー指示「並列で開発」)**:
  1. `src/wiki.rs`を新設: `WikiPage { id, project_id, slug, title,
     revisions: Vec<WikiRevision> }`と`WikiStore`(既存の`project.rs`/
     `comments.rs`と同じJSONファイル永続化パターン、`wiki.json`)。
     編集のたびに`WikiRevision`を`revisions`へ追記し、旧内容は保持する
     (差分表示は今回スコープ外、最小限の履歴保持のみ)。
  2. `main.rs`に`POST/GET /api/projects/:id/wiki`
     (一覧=`Need::View`、作成=`Need::Edit`、`slug`はプロジェクト内で
     一意)・`GET/PUT/DELETE /api/wiki/:id`(取得=`Need::View`、
     改訂=`Need::Edit`、削除=管理者のみ)を追加。既存の
     `comments.rs`/`access.rs`の権限モデルをそのまま再利用
     (複数パスパラメータ〈`:id/:slug`〉の実績がこのコードベースに
     無かったため、リスクを避けて単一パラメータ設計〈`/api/wiki/:id`〉
     に統一——一覧のみプロジェクトIDで、詳細操作はWikiページ自身の
     連番IDで行う)。
  3. テスト追加: `wiki.rs`のストレージ往復・`latest()`ヘルパー3件、
     ハンドラレベルで`wiki_page_lifecycle_is_gated_by_project_access_
     and_keeps_revision_history`(未ログイン401・無許可403・重複slug
     400・管理者による改訂で履歴が2件に増えること・削除後は404、を
     一気通貫で検証)。
  4. **検証**: `cargo build`警告1件(`WikiPage::latest()`が現状呼ばれて
     いないという`dead_code`警告のみ、既存の`AccelBackend`等未実装
     拡張点と同じ許容パターン)。`cargo test` **33件全green**
     (前回28件+今回5件〈wiki.rs単体4件・handler_tests 1件〉)。
  - 次にすべきこと: (1) 実SMTP環境でのOTPログイン→Wikiページ作成の
    フルE2E(今回もハンドラレベルテストでの代替検証に留まる)、
    (2) ガントチャート・カレンダー、(3) `aruaru-db`/PostgreSQL DUAL DB
    構成への移行(現状はJSONファイル永続化)、(4) VPSへのデプロイ
    (今回は未実施)。

- **2026-07-21 プロジェクト新設(器のみ)**: GitHub空リポジトリ・
  VPS空フォルダ・ローカル作業フォルダを用意。次回、`RS-Git`と同じ構成
  (`Cargo.toml`+`poem`)でのブートストラップに着手する。
  - 次にすべきこと: (1) 3プロジェクトのうちどれから着手するか決定、
    (2) Redmineの機能のうちMVP範囲の選定(チケット管理のみ、等)、
    (3) `RS-Git`と同じ認証・アクセス制御の再利用可否の検討、
    (4) `aruaru-db`との接続方式の設計。

- **2026-07-21(続き) v0.1.0ブートストラップ完了: チケットCRUD+OTP認証**
  (ユーザー指示「RS-Chikettoから着手」、`RS-Git`と`RS-Chiketto`のブート
  ストラップを並行して進めた1つ):
  1. `RS-Git`の`src/auth.rs`/`src/mail.rs`をそのまま移植(OTPログイン機構、
     環境変数名のみ`RSCHIKETTO_*`に変更)。v0.1.0時点では管理者アカウント
     のみログイン可能(`RS-Git`にある登録アカウント制・アクセス制御の
     細分化はまだ移植していない、次回以降の増分)。
  2. チケット(Issue)のCRUD: `POST/GET /api/tickets`・
     `GET/PUT /api/tickets/:id`。ステータスは`open`/`in_progress`/
     `closed`の3値。永続化はJSONファイル(`aruaru-db`/PostgreSQL DUAL DB
     構成への移行はまだ未着手——今回は動くMVPを優先)。
  3. **検証**: `cargo build`警告0件、`cargo test` 6件全green
     (auth関連、`RS-Git`からそのまま移植したテスト)。実バイナリで
     E2E確認: 未ログインでの`GET /api/tickets`→`401`、実SMTP経由の
     OTPログイン→チケット作成(`201`)→一覧取得→ステータス更新
     (`open`→`closed`)まで実HTTPで一連の動作を確認済み。
  - 次にすべきこと: (1) プロジェクト・サブプロジェクト階層の追加、
    (2) `RS-Git`にある登録アカウント制・アクセス制御(閲覧/編集の個別
    許可)の移植、(3) Wiki・ガントチャート等の追加機能、(4) `aruaru-db`/
    PostgreSQL DUAL DB構成への移行(現状はJSONファイル永続化)、
    (5) GitHubへの初回push・VPSデプロイ。

- **2026-07-21(続き) 登録アカウント制・アクセス制御を`main.rs`へ配線
  (`RS-Git`の設計をそのまま踏襲、上記(2)の着手分、コミット`53d4cb8`)**:
  1. `mod access; mod accounts;`を追加、`AppState`に
     `accounts_locked`(`RSCHIKETTO_ACCOUNTS_LOCKED`、既定`true`——
     `RS-Git`の`RGIT_ACCOUNTS_LOCKED`と同じ方針)を追加。
  2. `Ticket`に`project: String`(単純な文字列ラベル)を追加し、
     `access::is_allowed`経由で閲覧(`GET /api/tickets`は所属
     プロジェクトごとにフィルタ、`GET /api/tickets/:id`は403/401)・
     編集(`POST`/`PUT`)にアクセス制御を適用。プロジェクト名から
     `access.rs`の`project_id: u64`への変換は`DefaultHasher`による
     ハッシュ値(v0.1.0時点ではProject自体のCRUDは無い、正直な開示:
     ハッシュ衝突は理論上ゼロではないが実用上無視できる程度という
     判断——将来Project CRUDを追加する際は連番IDに置き換える)。
  3. `request_otp`を`accounts::AccountStore`の登録メールにも対応
     (管理者 OR 登録済みアカウント、`RS-Git`と同じ判定)。
  4. `POST/GET /api/accounts`・`POST /api/accounts/request`
     (認証不要)・`GET /api/accounts/requests`・
     `POST /api/accounts/requests/:id/decide`を`RS-Git`と同じ形状で追加。
     `decide`は承認時に`project`が指定されていればそのプロジェクトの
     `access::AccessConfig::accounts`へ閲覧/編集許可を書き込む。
     `accounts_locked`中は管理者メール以外の登録・承認申請の承認を
     `403`で拒否。
  5. `mail.rs`に`send_access_request_notice`/`send_access_decision`を
     追加(申請受付時に管理者へ、審査結果を申請者へSMTP通知、
     `RS-Git`と同じ)。
  6. **検証**: `cargo build`警告0件。`cargo test` **12件全green**
     (既存9件+`accounts`モジュール新規2件〈JSON永続化の往復・
     ファイル未存在時のデフォルト読み込み〉+既存の重複を除く)。
     **正直な開示**: 今回追加した`accounts`モジュールの単体テストは
     ストレージ層(JSON往復)のみで、HTTPハンドラレベルの統合テスト
     (ログイン可否・401/403の切り分け・承認フロー)は今回書いていない
     ——実バイナリでのcurlスモークテストで代替検証した(下記)。
     次回、`poem`のテストクライアントを使ったハンドラレベルの
     自動テストを追加すべき。
     実バイナリ起動(`RSCHIKETTO_DATA_DIR`一時ディレクトリ、
     `RSCHIKETTO_ADMIN_EMAIL=admin@example.com`、SMTP未設定)での
     `curl`スモークテスト: `GET /healthz`→`200`、`GET /api/tickets`
     (未ログイン)→`200`(空配列、フィルタ設計通り)、
     `POST /api/auth/request-otp`(未登録メール)→`403`、
     (管理者メール、SMTP未設定)→`503`、
     `POST /api/accounts/request`(認証不要)→`201`、
     `POST/GET /api/accounts`(未認証)→ともに`401`、を確認。
     **SMTPが無い環境のため、実OTPメール送受信を伴うログイン成功
     パス・`decide_access_request`の承認フルE2Eは未検証**(コード
     レビューと401/403系の実HTTP確認までに留まる、正直な開示)。
  - 次にすべきこと: (1) 実SMTP環境でのOTPログイン→チケット作成
    フルE2E(登録アカウント・自己申請承認を含む)、(2) `poem`テスト
    クライアントによるハンドラレベルの自動テスト追加、
    (3) Project自体のCRUD(現状は文字列ラベル+ハッシュのみ)、
    (4) VPSへのデプロイ(今回は未実施)、(5) Wiki・ガントチャート等
    の追加機能、(6) `aruaru-db`/PostgreSQL DUAL DB構成への移行。

- **2026-07-21(続き) `poem::test::TestClient`によるハンドラレベル統合
  テストを追加(上記(2)の宿題への対応)**:
  1. `main.rs`のルーティング定義を`build_routes(state: AppState) ->
     impl poem::Endpoint`として切り出し、`main()`とテストの両方から
     再利用できるようにした。
  2. `Cargo.toml`の`poem`依存に`features = ["test"]`を追加
     (`poem::test::TestClient`を使うために必須、当初
     `unresolved import poem::test`でビルド失敗していたため修正)。
  3. `#[cfg(test)] mod handler_tests`を`main.rs`末尾に追加、4件:
     - 未認証`GET /api/tickets`→`200`・空配列(既存のプロジェクト単位
       フィルタ設計通り、401ではないことを確認)。
     - `POST /api/accounts/request`(自己申請・認証不要)→`201`、
       `pending_requests`に登録されることを確認。
     - 管理者セッションで`decide`承認→`access::AccessConfig`へ
       期待した`allow_view`/`allow_edit`が書き込まれることを確認。
     - `accounts_locked=true`時、管理者以外の承認対象を管理者セッションで
       承認しようとすると`403`になることを確認(`AppState`をテスト
       ローカルに構築、プロセス環境変数`RSCHIKETTO_ACCOUNTS_LOCKED`は
       変更していない)。
     各テストは`std::env::temp_dir()`配下に一意な一時ディレクトリを
     `data_root`として使い、テスト間の状態共有を避けている。
  4. **検証**: `cargo build`警告0件、`cargo test` **16件全green**
     (既存12件+今回追加4件)。
  - 次にすべきこと: (1) 実SMTP環境でのOTPログイン→チケット作成
    フルE2E、(2) Project自体のCRUD、(3) VPSへのデプロイ(今回は未実施)、
    (4) Wiki・ガントチャート等の追加機能、(5) `aruaru-db`/PostgreSQL
    DUAL DB構成への移行。


- **2026-07-22 Project自体のCRUDを追加(HANDOFF記載の宿題(3)への対応)**:
  1. `src/project.rs`を新設: `Project { id: u64, name: String,
     description: String, created_at: String, updated_at: String }`と
     `ProjectStore`(既存の`TicketStore`/`accounts::AccountStore`と同じ
     JSONファイル永続化パターン、`projects.json`)。
  2. `main.rs`に`POST/GET /api/projects`・`GET/PUT/DELETE /api/projects/:id`
     を追加。作成・更新・削除は`require_admin_session`で管理者のみに
     制限(`access.rs`の「構造を作れるのは管理者のみ」という既存方針を
     踏襲)、一覧・詳細取得は認証不要(プロジェクトの存在自体は隠す
     情報ではなく、チケットの中身は`access.rs`のアクセス制御で個別に
     守られる、という判断)。
  3. `Ticket.project: String`(文字列ラベル)を`Ticket.project_id: u64`
     (実在する`Project`への参照)に置き換え。`create_ticket`で
     `project::ProjectStore::exists`により実在確認し、存在しない
     `project_id`の場合は`400`で明確に拒否するようにした。
  4. `check_project_access`・`access.rs`連携から`project_id()`関数
     (`DefaultHasher`によるハッシュ経由の変換)を削除し、実在する
     `Project.id`(連番`u64`)を直接`access::load`/`access::save`へ渡す
     ように変更(HANDOFFに記載の「将来Project CRUDを追加する際は
     連番IDに置き換える」を実施)。`decide_access_request`の
     `DecideAccessRequestPayload.project: Option<String>`も
     `project_id: Option<u64>`に変更。
  5. テスト追加: `project.rs`のストレージ往復テスト2件、
     ハンドラレベルで`project_crud_via_http`(管理者のみ作成・更新・
     削除できること、一覧・詳細は認証不要であること)、
     `create_ticket_against_nonexistent_project_fails_cleanly`
     (存在しない`project_id`でのチケット作成が`400`になること)、
     `access_control_gates_ticket_creation_by_real_project_id`
     (実在の連番`project_id`に対して`access::AccessConfig`が正しく
     効くこと、未ログイン`401`・無許可アカウント`403`・許可済み
     アカウント`201`の3パターン)。
  6. `README.md`にAPIエンドポイント一覧表を新設(従来README側には
     エンドポイント一覧が無かったため今回新設、`GET /`ランディング
     ページの表と同内容に揃えた)。
  7. **検証**: `cargo build`警告0件。`cargo test` **22件全green**
     (前回HANDOFF時点の16件+今回の6件〈project.rs 2件・
     handler_tests 4件〉、なお前回16件から今回着手時点で
     `handler_tests`の既存テストが1件`project`関連の変更で調整済み
     ―新規追加分は正味6件)。実バイナリでのcurlスモークテスト
     (`RSCHIKETTO_DATA_DIR`一時ディレクトリ、SMTP未設定): `GET /api/projects`
     (未認証)→`200`・空配列、`POST /api/projects`(未認証)→`401`、
     `POST /api/tickets`(存在しない`project_id=999999`)→`400`
     (期待通りのエラーメッセージ)、`GET /api/tickets`(未認証)→`200`・
     空配列、を確認。**正直な開示**: SMTPが無いローカル検証環境のため、
     管理者OTPログインを経由した「プロジェクト作成→そのproject_idで
     チケット作成→アクセス制御が効く」というフル経路のcurl E2Eは
     今回も未検証(前回HANDOFFと同じ制約)——この経路は
     `access_control_gates_ticket_creation_by_real_project_id`の
     ハンドラレベルテスト(`AuthStore::create_session`でOTPを迂回して
     セッションを直接発行、既存テストと同じ手法)で代替検証している。
  - 次にすべきこと: (1) 実SMTP環境でのOTPログイン→プロジェクト作成→
    チケット作成のフルE2E、(2) プロジェクトのサブプロジェクト階層
    (親子関係)、(3) VPSへのデプロイ、(4) Wiki・ガントチャート等の
    追加機能、(5) `aruaru-db`/PostgreSQL DUAL DB構成への移行。

- **2026-07-22(続き) プロジェクトのサブプロジェクト階層・チケットへの
  コメントを追加(前回HANDOFF宿題(2)、および実用最小限として優先度が
  高いと判断したコメント機能への対応)**:
  1. `project.rs`の`Project`に`parent_id: Option<u64>`を追加。
     `ProjectStore::children_of`(直接の子一覧)・`would_create_cycle`
     (自分自身や自分の子孫を親に設定しようとする循環参照を検出)を実装。
  2. `main.rs`: `POST /api/projects`・`PUT /api/projects/:id`が
     `parent_id`を受け付けるように変更(`PUT`側は二重`Option`
     デシリアライズパターン——フィールド省略は変更なし、`null`は
     親解除、値ありは親設定——を新規導入)。`GET /api/projects/:id/children`
     を追加(認証不要、既存の一覧・詳細取得と同じ方針)。循環参照・
     存在しない`parent_id`はいずれも`400`で拒否。
  3. `src/comments.rs`を新設: `Comment { id, ticket_id, author_email,
     body, created_at }`と`CommentStore`(既存パターンと同じJSON
     ファイル永続化、`comments.json`)。`GET/POST /api/tickets/:id/comments`
     (閲覧/編集権限をチケット所属プロジェクトの`access.rs`経由で
     チェック、既存の`update_ticket`/`get_ticket`と同じ権限モデルを
     再利用——モデレーションキューは投稿時点で権限確認済みのため不要)、
     `DELETE /api/comments/:id`(管理者または投稿者本人のみ)を追加。
  4. `README.md`・`GET /`ランディングページのエンドポイント表、および
     このCLAUDE.mdの正直な開示リストから「サブプロジェクト階層」の
     未実装項目を除去。
  5. テスト追加: `project.rs`の`would_create_cycle_detects_self_and_ancestor_cycles`、
     `comments.rs`のストレージ往復2件、ハンドラレベルで
     `subproject_hierarchy_children_listing_and_cycle_rejection`
     (子作成・`GET /children`・親を自分の子孫や自分自身に設定しようと
     すると`400`)、`comment_creation_is_gated_by_project_edit_access`・
     `comment_visibility_is_gated_by_project_view_access`
     (未ログイン`401`、無許可アカウント`403`、許可済みアカウント成功)。
  6. **検証**: `cargo build`(`cargo build`単体・`cargo build`の一部の
     `cargo test`両方)警告0件。`cargo test` **28件全green**(前回22件+
     今回6件〈project.rs 1件・comments.rs 2件・handler_tests 3件〉)。
     実バイナリを起動(`RSCHIKETTO_DATA_DIR`一時ディレクトリ、
     `RSCHIKETTO_ADMIN_EMAIL=admin@example.com`、SMTP未設定、
     `RSCHIKETTO_PORT=8199`)して`curl`で実HTTP確認: `GET /healthz`→
     `200`、`GET /api/projects/1/children`(存在しない`id`)→`404`、
     `POST /api/projects`(未認証)→`401`、`GET /api/tickets/1/comments`
     (存在しないチケット)→`404`、`POST /api/tickets/1/comments`
     (存在しないチケット、未認証)→`404`(存在チェックが認証チェックより
     先に走る設計通り)、をいずれも確認。**正直な開示**: 前回までと
     同じ制約でSMTPが無いローカル検証環境のため、管理者OTPログインを
     経由した「プロジェクト作成→子プロジェクト作成→チケット作成→
     コメント投稿」というフル経路のcurl E2Eは今回も未検証——この経路は
     上記のハンドラレベルテスト(`AuthStore::create_session`でOTPを
     迂回してセッションを直接発行)で代替検証している。
  - 次にすべきこと: (1) 実SMTP環境でのフルE2E(OTPログイン→プロジェクト
    作成→サブプロジェクト作成→チケット作成→コメント投稿)、
    (2) VPSへのデプロイ(今回も未実施)、(3) Wiki・ガントチャート等の
    追加機能、(4) `aruaru-db`/PostgreSQL DUAL DB構成への移行、
    (5) コメントの編集(現状は投稿・削除のみ)。

## 同時並行開発の対象プロジェクト(2026-07-21、ユーザー指示・拡張版)

`RS-Chiketto`・`RS-Blog`・`RS-EC`(この3プロジェクト自身、着手順は
「1つずつ順番に」の方針のまま)に加えて、以下の既存プロジェクトを
**同時に開発を進め、完成度を高めていく**:

- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの
  正本。3プロジェクトの`CLAUDE.md`もここの記述と同期を取る。
- [aruaru-db](https://github.com/aon-co-jp/aruaru-db) — ZFS互換・ACID
  互換のRust製DB。3プロジェクトが採用する「分身の術」DB共有構成の実体。
- [open-cuda](https://github.com/aon-co-jp/open-cuda) — GPU抽象化・
  GEMM/Attention計算基盤(`opencuda-blas`/`opencuda-bert`)。
- [aruaru-llm](https://github.com/aon-co-jp/aruaru-llm) — 上記
  `open-cuda`を使った実装例(bag-of-words→文埋め込みベースの意図分類へ
  移行済み)。3プロジェクトが将来AI機能を持つ際の先行実装として参照。
- [open-web-server](https://github.com/aon-co-jp/open-web-server) —
  「分身の術」構成(1インスタンスを複数ドメインが共有)の基盤実装、
  Nginx/Apacheハイブリッド仕様のWebサーバー。
- [open-cosmo](https://github.com/aon-co-jp/open-cosmo) — 関連する
  Webサーバー/フロントエンド基盤(詳細は同リポジトリのCLAUDE.md参照)。
- [RPoem](https://github.com/aon-co-jp/RPoem) — アプリケーションサーバー
  層(旧poem-cosmo-tauri)。`open-raid-z`とVersionlessAPIによる
  バージョンレス運用、`aruaru-db`とのDUAL DATABASE構成の先行実装。

- Python製AIライブラリのRust移植ハイブリッド/トライブリッド版
  (マーケティング調査での1〜6位、vLLM/Transformers/NumPy/PyTorch互換/
  scikit-learn/Whisper相当の良いとこ取り)——**Rustを基本とし、必要なら
  `RPoem`(アプリケーションサーバー層)も併用する**(ユーザー指示、
  2026-07-21追記)。`open-cuda`ワークスペース内の`opencuda-blas`
  (NumPy相当)・`opencuda-bert`(Transformers推論パス相当、実装済み)が
  このトライブリッド化の実体。今後の`opencuda-llm`(vLLM相当、生成
  デコーダ追加時)を、必要であれば`RPoem`上のHTTPサービスとして
  提供することも視野に入れる。

**理由**: これらは3プロジェクトが実際に依存する基盤コンポーネント
(DB層・GPU計算基盤・「分身の術」共有構成・アプリケーションサーバー層)
であり、基盤側の完成を待ってから3プロジェクトに着手するのではなく、
実際に統合しながら並行して育て、エコシステム全体の完成度を高めていく
方針とする。
