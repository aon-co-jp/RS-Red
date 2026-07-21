# 開発方針＆開発環境ルール(RRedmine)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RRedmine](https://github.com/aon-co-jp/RRedmine)。
VPS上の作業パス: `/root/RRedmine`(空フォルダ作成済み、2026-07-21)。

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
  参照——RRedmine側で構造化データ処理が必要になった際の候補。

## Redmineの主要機能(着手時の優先順位付けの参考)

- チケット(Issue)管理・ワークフロー・カスタムフィールド
- プロジェクト・サブプロジェクト階層
- ガントチャート・カレンダー
- Wiki・フォーラム
- リポジトリ連携(SCM閲覧)
- ユーザー・ロール・権限管理

## HANDOFF

- **2026-07-21 プロジェクト新設(器のみ)**: GitHub空リポジトリ・
  VPS空フォルダ・ローカル作業フォルダを用意。次回、`RGit`と同じ構成
  (`Cargo.toml`+`poem`)でのブートストラップに着手する。
  - 次にすべきこと: (1) Redmineの機能のうちMVP範囲の選定(チケット
    管理のみ、等)、(2) `RGit`と同じ認証・アクセス制御の再利用可否の
    検討、(3) データモデル設計(DB選定含む)。
