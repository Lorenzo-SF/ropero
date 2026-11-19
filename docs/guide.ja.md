# Ropero ユーザーガイド

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · **日本語** · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero は、複数のエンジンでローカル AI モデルをインストール、設定、実行する Rust 製 CLI です。選択したエンジンと依存関係を準備し、サービスを起動・監視します。エンジンが対応している場合は OpenAI 互換 API を提供します。Ropero v2 は開発中です。

## はじめに

リポジトリのルートでビルドし、コマンドを確認します。

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

engine コマンドで利用可能なエンジンを一覧表示できます。直接指定することもできます（例: `ropero install engines basalt`）。エンジンのインストールではランタイムと依存関係を準備します。モデルは `ropero install models <model>` で別途インストールします。

CLI の初期言語は英語です。スペイン語を保存するには `ropero config --lang ES`、英語に戻すには `ropero config --lang EN` を実行します。

## カタログと TOML

`engines/` のマニフェストには要件、検出、インストール、機能が記載されています。`models/<engine>/` のファイルにはモデル、形式、ダウンロード、成果物、実行オプションが定義されています。通常、使用中のモデル定義は `~/.config/ropero/models/<engine>/`、ユーザー用エンジンマニフェストは `~/.config/ropero/engines/` に保存されます。リポジトリ内の TOML は参考例であり、自動では読み込まれません。

```bash
ropero models add
ropero models add --path model.toml
ropero install models <model>
ropero <model>
ropero status --live
```

## エンジンと制限

- **llama-server** と **turbo-llama** は GGUF モデルを実行します。
- **FastFlowLM** は `flm` CLI に処理を委譲します。形式や NPU 対応は FastFlowLM と XRT に依存します。
- **Ignis** はダウンロードしたエンジン成果物を実行します。
- **Basalt** は `.basalt` パックを実行します。Ropero での対応は Linux x86-64 のみです。エンジンのインストールに成功すると、未登録の場合に Qwen3.8-Flash-Next の TOML をユーザーカタログに登録します。モデルパックは別途インストールします。AVX2 CPU、NVIDIA Blackwell GPU、対応ドライバーが必要です。
- **Strata** は準備済みプロファイルを実行します。Ropero はプロファイルを作成・ダウンロードしません。
- **vLLM**、**MLX-Serve**、**TensorRT-LLM** にはアダプターがありますが、対応状況は OS とモデルによって異なります。MLX-Serve には macOS と Apple Silicon が必要です。
- **AirLLM** の対応は限定的で、サーバーアダプターはありません。

要件と既知の制限は[エンジン互換性ガイド（英語）](engine-support.en.md)を参照してください。

## エージェント、IDE、ツール

`ropero connect --list` で OpenCode、Codex、Claude Code、Pi、Hermes、OpenClaw、PicoClaw、VS Code などの自動接続先を確認できます。接続前に Ropero でモデルを起動してください。接続機能はクライアントを設定するもので、エージェントのインストールやエンジンの起動は行いません。

Zed は OpenAI 互換エンドポイントに対応し、キーをシステムのキーチェーンに保存します。Cursor の公式資料では Ropero のローカルエンドポイントへの直接接続は確認できません。MicroClaw は設定の `llm_base_url` を利用できます。Graphify は互換アシスタントと併用する任意のリポジトリ分析ツールで、Ropero のエンジンやモデルではありません。

[連携ガイド（英語）](agent-integrations.en.md)と[CLI リファレンス（英語）](especificacion/ropero-cli.en.md)も参照してください。

## モデルの有効化と無効化

`ropero models` は既定で、インストール済みエンジンとすぐ使えるモデルを表示します。利用可能なモデルがないインストール済みエンジンも、メッセージ付きで表示されます。`--all` はカタログ全体、`--fired` は無効化したモデルを表示します。無効化・再有効化では TOML をエンジンの `fired/` フォルダーへ移動し、定義やモデルファイルは削除しません。有効なモデルと名前が重複する場合、再有効化の前に一意な新しい名前を尋ねます。

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
