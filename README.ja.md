![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**AIモデルをローカルにインストールして実行します。**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · **日本語** · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · [Русский](README.ru.md)

Roperoはローカルモデルと、その実行エンジンを管理します。モデルを選ぶと、宣言されたエンジンを準備してサービスを起動し、利用可能か確認します。エンジンが対応している場合はOpenAI互換APIを提供します。

## 主な機能

- エンジン（`ropero install engines`）とモデル（`ropero install models`）を別々にインストール。
- 要件、ダウンロード、実行設定をTOMLで宣言するカタログ。
- モデルサービスの起動、状態確認、停止。
- 診断、設定情報、起動コマンド、評価ツール。
- 対応するコーディングアシスタントとの連携。各連携の制約を明記。

## はじめに

このリポジトリは開発中のRopero v2です。ルートディレクトリでビルドします。

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

エンジン一覧を表示するには`ropero install engines`を実行します。たとえば`ropero install engines basalt`で選択できます。モデルは別途`ropero install models <モデル名>`でインストールします。

## モデルと使い方

`engines/`のマニフェストはエンジンとインストール方法を記述し、`models/<engine>/`にはモデルのTOML定義があります。通常のカタログは`~/.config/ropero/models/`です。このリポジトリ内のTOMLは参考例であり、自動では読み込まれません。

```bash
ropero models add
ropero models add --path model.toml
ropero <モデル名>
ropero status --live
```

詳しい説明は[スペイン語](docs/engine-support.es.md)と[英語](docs/engine-support.en.md)のガイド、[CLIリファレンス](docs/especificacion/ropero-cli.en.md)、[連携ガイド](docs/agent-integrations.en.md)を参照してください。[日本語のユーザーガイド](docs/guide.ja.md)も参照してください。ユーザーガイドは10言語で提供し、互換性と連携の詳細リファレンスは現在英語とスペイン語です。
