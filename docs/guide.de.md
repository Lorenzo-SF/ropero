# Ropero-Handbuch

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · **Deutsch** · [Italiano](guide.it.md) · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero ist eine Rust-CLI zum Installieren, Konfigurieren und Ausführen lokaler KI-Modelle mit verschiedenen Engines. Ropero richtet die deklarierte Engine ein, startet und überwacht deren Dienst und stellt eine OpenAI-kompatible API bereit, sofern die Engine dies unterstützt.

## Einstieg

Ropero v2 befindet sich noch in Entwicklung. Im Stammverzeichnis des Repositories:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

`ropero install engines` zeigt installierbare Engines. Du kannst eine direkt wählen, zum Beispiel `ropero install engines basalt`. Dabei werden Runtime und Abhängigkeiten der Engine vorbereitet. Das Modell installierst du separat mit `ropero install models <modell>`.

Die CLI verwendet standardmäßig Englisch. Mit `ropero config --lang ES` speicherst du Spanisch als Oberflächensprache; `ropero config --lang EN` stellt Englisch wieder her.

## Katalog und TOML

Manifeste in `engines/` beschreiben Anforderungen, Erkennung, Installation und Fähigkeiten. Dateien unter `models/<engine>/` definieren Modell, Format, Download, Artefakte und Laufzeitoptionen. Bei einer normalen Installation liegen aktive Modelle in `~/.config/ropero/models/<engine>/` und eigene Engine-Manifeste in `~/.config/ropero/engines/`. Die TOML-Dateien im Repository sind Beispiele und werden nicht automatisch geladen.

```bash
ropero models add
ropero models add --path modell.toml
ropero install models <modell>
ropero <modell>
ropero status --live
```

Für Basalt bereitet `ropero install engines basalt` die Runtime vor; das Modell-Pack wird separat installiert. Basalt benötigt Linux x86-64, eine AVX2-CPU, eine NVIDIA-Blackwell-GPU und einen kompatiblen Treiber. Das Beispiel für Qwen3.8-Flash-Next liegt in `models/basalt/`; Hugging-Face-Downloads können die CLI `hf` und eine Anmeldung erfordern.

## Engines und Grenzen

- **llama-server** und **turbo-llama** bedienen GGUF-Modelle. Turbo-Llama ist ein Fork; prüfe die Optionen der installierten Version.
- **FastFlowLM** delegiert Befehle an die CLI `flm`; Format- und NPU-Unterstützung hängen von FastFlowLM und XRT ab.
- **Ignis** lädt eigene Artefakte und bedient sie aus dem konfigurierten Artefaktverzeichnis.
- **Basalt** stellt `.basalt`-Packs bereit. Ropero unterstützt es nur unter Linux x86-64. Nach erfolgreicher Engine-Installation registriert Ropero die mitgelieferte Qwen3.8-Flash-Next-TOML-Datei, falls sie fehlt; das Modell-Pack wird separat installiert.
- **Strata** bedient vorbereitete Profile; Ropero erstellt oder lädt keine Profile.
- **vLLM**, **MLX-Serve** und **TensorRT-LLM** besitzen Adapter, wurden auf diesem Rechner aber nicht mit einem echten Start geprüft. MLX-Serve benötigt macOS und Apple Silicon.
- **AirLLM** ist nur teilweise integriert: Ropero hat keinen Serveradapter dafür.

Die Unterstützung hängt von Betriebssystem, Hardware, Version und Modell ab. Lies die ausführlichen Kompatibilitätsleitfäden auf [Englisch](engine-support.en.md) oder [Spanisch](engine-support.es.md).

## Agenten, IDEs und Werkzeuge

`ropero connect --list` zeigt automatische Integrationen. Dazu gehören OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw und VS Code. Starte das Modell zuerst mit Ropero; die Verbindung konfiguriert den Client, installiert aber weder den Agenten noch startet sie die Engine.

Zed unterstützt OpenAI-kompatible Endpunkte und speichert Schlüssel im System-Schlüsselbund; richte es in Agent Settings ein. Die offizielle Cursor-Dokumentation bestätigt keine direkte Verbindung zu lokalen Ropero-Endpunkten. MicroClaw kann `llm_base_url` verwenden; schütze die Konfigurationsdatei mit Tokens. Graphify ist ein optionales Werkzeug, das die Repository-Analyse über einen kompatiblen Assistenten erweitert; es ist weder Ropero-Engine noch Modell.

Weitere Hinweise stehen in der [Integrationsanleitung](agent-integrations.en.md) und der [CLI-Referenz](especificacion/ropero-cli.en.md).

## Modelle aktivieren und deaktivieren

`ropero models` zeigt standardmäßig installierte Engines und sofort nutzbare Modelle. Eine installierte Engine ohne verfügbare Modelle wird mit einem Hinweis angezeigt. Mit `--all` sehen Sie den gesamten Katalog, mit `--fired` die deaktivierten Modelle. Deaktivieren und Reaktivieren verschiebt die TOML-Datei in den oder aus dem Ordner `fired/` der Engine; die Definition und Modelldateien bleiben erhalten. Ist der Name bereits aktiv, fragt Ropero vor dem Reaktivieren nach einem eindeutigen neuen Namen.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
