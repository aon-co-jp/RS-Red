# 開発方針＆開発環境ルール(RS-Chiketto)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RS-Chiketto](https://github.com/aon-co-jp/RS-Chiketto)。
VPS上の作業パス: `/root/RS-Chiketto`(空フォルダ作成済み、2026-07-21)。

## このプロジェクトの役割

[Redmine](https://redmine.org/)(実際にはRuby on Rails製)の、
ハイスピード・ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)
(RPoem)版を目指す。`RGit`(Gitea相当)・`RJSON`(JSON処理)と同じ
`aon-co-jp`エコシステムの一員。

> ⚠️ **正直な開示**: 2026-07-21時点でコード未着手(このCLAUDE.mdのみの
> 状態)。このエコシステム共通の方針として、実装が追いつくまでは
> 「Redmineの代替品」を名乗らず、進捗をこのHANDOFFに正直に記録する。

## 着手時に踏襲すべき既存プロジェクトの設計方針

- **`RGit`**(git smart HTTP・OTPログイン・アクセス制御・容量ベースの
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
  **1つずつ順番に、`RGit`と同じ深さまで作り込んでから次へ**進める。
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

- **公開パス**: `runo.tokyo/chiketto`(`RGit`の`runo.tokyo/rgit`と同じ
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

- **2026-07-21 プロジェクト新設(器のみ)**: GitHub空リポジトリ・
  VPS空フォルダ・ローカル作業フォルダを用意。次回、`RGit`と同じ構成
  (`Cargo.toml`+`poem`)でのブートストラップに着手する。
  - 次にすべきこと: (1) 3プロジェクトのうちどれから着手するか決定、
    (2) Redmineの機能のうちMVP範囲の選定(チケット管理のみ、等)、
    (3) `RGit`と同じ認証・アクセス制御の再利用可否の検討、
    (4) `aruaru-db`との接続方式の設計。

- **2026-07-21(続き) v0.1.0ブートストラップ完了: チケットCRUD+OTP認証**
  (ユーザー指示「RS-Chikettoから着手」、`RGit`と`RS-Chiketto`のブート
  ストラップを並行して進めた1つ):
  1. `RGit`の`src/auth.rs`/`src/mail.rs`をそのまま移植(OTPログイン機構、
     環境変数名のみ`RSCHIKETTO_*`に変更)。v0.1.0時点では管理者アカウント
     のみログイン可能(`RGit`にある登録アカウント制・アクセス制御の
     細分化はまだ移植していない、次回以降の増分)。
  2. チケット(Issue)のCRUD: `POST/GET /api/tickets`・
     `GET/PUT /api/tickets/:id`。ステータスは`open`/`in_progress`/
     `closed`の3値。永続化はJSONファイル(`aruaru-db`/PostgreSQL DUAL DB
     構成への移行はまだ未着手——今回は動くMVPを優先)。
  3. **検証**: `cargo build`警告0件、`cargo test` 6件全green
     (auth関連、`RGit`からそのまま移植したテスト)。実バイナリで
     E2E確認: 未ログインでの`GET /api/tickets`→`401`、実SMTP経由の
     OTPログイン→チケット作成(`201`)→一覧取得→ステータス更新
     (`open`→`closed`)まで実HTTPで一連の動作を確認済み。
  - 次にすべきこと: (1) プロジェクト・サブプロジェクト階層の追加、
    (2) `RGit`にある登録アカウント制・アクセス制御(閲覧/編集の個別
    許可)の移植、(3) Wiki・ガントチャート等の追加機能、(4) `aruaru-db`/
    PostgreSQL DUAL DB構成への移行(現状はJSONファイル永続化)、
    (5) GitHubへの初回push・VPSデプロイ。


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
