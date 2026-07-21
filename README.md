# RS-Chiketto

[Redmine](https://redmine.org/)のハイスピード・ハイセキュリティ・省メモリな
Rust+[poem](https://github.com/poem-web/poem)版。

> ⚠️ v0.1.0時点ではチケット(Issue)CRUDのみ。詳細は`CLAUDE.md`参照。

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
