# PORTING.md — RS-Red を他プロジェクトへお引越しする際のガイド

## 現状(2026-07-23)

チケット(Issue)CRUD・プロジェクトCRUD+サブプロジェクト階層・コメント・
Wiki(改訂履歴保持)・OTP認証・プロジェクト単位アクセス制御・ブラウザGUI
(Rust→WebAssembly)まで実装済み。詳細は`CLAUDE.md`のHANDOFF参照。

## 1. `RS-Git`からそのまま移植したパターン

- `auth.rs`(OTPログイン機構)・`mail.rs`(SMTP送信)は`RS-Git`の実装を
  そのまま移植したもの(環境変数名のみ`RSCHIKETTO_*`にリネーム)。
- 登録アカウント制(`accounts_locked`、既定`true`)・アクセス制御
  (`access.rs`、閲覧/編集の個別許可)も`RS-Git`と同じ設計を踏襲。

## 2. RustJSON経由の永続化(2026-07-23移植)

`src/rustjson.rs`は[RPoem](https://github.com/aon-co-jp/RPoem)の
`open-runo-rustjson`crateを移植したもの(トレイリングカンマ・コメント・
裸キー・シングルクォート文字列を許容する緩い構文、パース結果は標準
`serde_json::Value`)。**クロスリポジトリのCargo依存(RPoem側crateへの
直接依存)は避け、小さなモジュールとして直接コピーする**——これは
`open-web-server`/`RPoem`のリリースCIで実際にpath依存問題が発生した
教訓に基づく判断(詳細は`open-raid-z/CLAUDE.md`・`PORTING.md`参照)。

移植パターン:
```rust
// 読み込み: 緩い構文を許容
Ok(bytes) => crate::rustjson::parse_typed(&bytes).unwrap_or_default(),
// 書き込み: 引き続き整形済み標準JSON(可読性維持、RustJSONの入力としても有効)
let bytes = serde_json::to_vec_pretty(store).expect("...");
```

## 3. ブラウザGUI(`web/`、Rust→WebAssembly)

「チケット管理を行うWEBアプリである以上GUIは基本機能」という方針
(2026-07-23、ユーザー指示)。Tauri・Node.js・TypeScriptには依存しない
(このエコシステム共通方針)。移植時のポイント:

- **`GET /`はGUIを優先、無ければAPI概要ページへフォールバック**する
  設計(`RSCHIKETTO_WEB_DIR`環境変数で配置場所を変更可能)。GUIビルド
  成果物が無い環境でも壊れない。
- **オンライン専用**(オフライン/Service Worker対応は行っていない、
  ユーザー確認済みのスコープ)。
- **ピンチズームは標準の`viewport`メタタグのみで機能する**——
  Android/iOSのモバイルブラウザ向けに特別な実装は一切不要。
- `GET /pkg/:file`ハンドラでWASM成果物を配信する際は、ファイル名に
  `..`・`/`・`\`を含む場合を拒否するパストラバーサル対策を必ず入れる
  (`open-web-server`の`static_files.rs`と同じ方針)。
- ビルド成果物(`pkg/*.js`・`pkg/*.wasm`)は`.gitignore`せず**コミット
  する**方針とした——GUIをこのアプリの基本機能と位置づけたため、
  `git clone`直後にwasmツールチェーン無しで動く体験を優先した
  (他リポジトリの一部で採用している「ビルド成果物はgitignore」方針
  とは意図的に異なる判断、理由をこのファイルに明記)。

## 4. 同時並行開発の対象プロジェクト

- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの正本
- [aruaru-db](https://github.com/aon-co-jp/aruaru-db) — DB層(「分身の術」共有構成、DUAL DB移行先候補)
- [open-cuda](https://github.com/aon-co-jp/open-cuda) — GPU計算基盤
- [open-web-server](https://github.com/aon-co-jp/open-web-server) — 「分身の術」基盤実装
- [RPoem](https://github.com/aon-co-jp/RPoem) — アプリケーションサーバー層、RustJSONの移植元

## 5. 未着手のまま残る移植候補(次回以降)

- ストレージ先選択機能(Googleドライブ・他クラウド・VPS、`StorageBackend`
  トレイト抽象化)——構想のみ、`CLAUDE.md`参照。
- `aruaru-db`/PostgreSQL DUAL DB構成への移行(現状はJSONファイルのみ)。
- ガントチャート・カレンダーのGUI実装。
