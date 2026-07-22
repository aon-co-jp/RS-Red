# RS-Chiketto

**開発開始日: 2026-07-21**(このリポジトリのGitHub作成日)

[Redmine](https://redmine.org/)のハイスピード・ハイセキュリティ・省メモリな
Rust+[poem](https://github.com/poem-web/poem)(RPoem)版。運用時はVPSレンタル
サーバー費用を安く抑えられる予定です。

> ⚠️ v0.1.0時点ではチケット(Issue)・プロジェクトのCRUDのみ。詳細は`CLAUDE.md`参照。

## API エンドポイント

| メソッド / パス | 説明 |
| --- | --- |
| `GET /healthz` | ヘルスチェック |
| `POST /api/auth/request-otp` | ログイン用ワンタイムパスワードをメール送信 |
| `POST /api/auth/verify-otp` | OTPを検証してセッショントークンを発行 |
| `POST /api/auth/logout` | ログアウト(トークン失効) |
| `GET /api/accounts` / `POST /api/accounts` | 登録アカウント一覧取得 / 追加(管理者のみ) |
| `POST /api/accounts/request` | アカウント利用の自己申請(認証不要) |
| `GET /api/accounts/requests` | 保留中の自己申請一覧(管理者のみ) |
| `POST /api/accounts/requests/:id/decide` | 自己申請の承認/却下・プロジェクトへの閲覧/編集権限付与(管理者のみ) |
| `GET /api/projects` / `POST /api/projects` | プロジェクト一覧取得(認証不要) / 新規作成(管理者のみ) |
| `GET /api/projects/:id` / `PUT /api/projects/:id` / `DELETE /api/projects/:id` | プロジェクト詳細取得(認証不要) / 更新・削除(管理者のみ) |
| `GET /api/tickets` / `POST /api/tickets` | チケット一覧取得(アクセス権のあるプロジェクトのみ) / 新規作成(実在する`project_id`が必要) |
| `GET /api/tickets/:id` / `PUT /api/tickets/:id` | チケット詳細取得 / 更新(ステータス変更含む) |

## インストール(ビルド済みバイナリ、インストーラー付き)

タグ付きリリース(`vX.Y.Z`)ごとに、GitHub Actions
(`.github/workflows/release.yml`)がLinux・Windows向けバイナリを
自動ビルドし、[GitHub Releases](https://github.com/aon-co-jp/RS-Chiketto/releases)へ添付する。

### Linux(AlmaLinux・Ubuntu・Debian・Fedora・RHEL等、systemdを使う主要ディストリ共通)

静的リンクされたmuslバイナリのため、ディストリ固有のライブラリ依存は無い。

```bash
curl -fsSL https://github.com/aon-co-jp/RS-Chiketto/releases/latest/download/rs-chiketto-linux-x86_64.tar.gz | tar xz
sudo ./install.sh
sudo systemctl edit rs-chiketto   # RSCHIKETTO_ADMIN_EMAIL等を設定
sudo systemctl enable --now rs-chiketto
```

### Windows / Windows Server

管理者権限のPowerShellで:

```powershell
Invoke-WebRequest -Uri "https://github.com/aon-co-jp/RS-Chiketto/releases/latest/download/rs-chiketto-windows-x86_64.zip" -OutFile rs-chiketto.zip
Expand-Archive rs-chiketto.zip -DestinationPath rs-chiketto
cd rs-chiketto
.\install.ps1
```

## ソースからビルド

```bash
cargo build --release
```

## ライセンス

Apache-2.0
