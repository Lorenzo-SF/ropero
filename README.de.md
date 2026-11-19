![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**KI-Modelle lokal installieren und ausführen.**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · **Deutsch** · [Italiano](README.it.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · [Русский](README.ru.md)

Ropero verwaltet lokale Modelle und die Engines, die sie ausführen. Wähle ein Modell: Ropero richtet die deklarierte Engine ein, startet den Dienst, prüft dessen Bereitschaft und stellt – sofern unterstützt – eine OpenAI-kompatible API bereit.

## Funktionen

- Engines (`ropero install engines`) und Modelle (`ropero install models`) getrennt installieren.
- Deklarativer TOML-Katalog für Anforderungen, Downloads und Laufzeitparameter.
- Modelldienste starten, überwachen und beenden.
- Diagnose, Konfigurationsinformationen, Startbefehle und Auswertungen.
- Integrationen für kompatible Programmierassistenten mit dokumentierten Einschränkungen.

## Einstieg

Dieses Repository enthält Ropero v2, das sich noch in Entwicklung befindet. Im Stammverzeichnis kompilieren:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

Der Engine-Befehl listet verfügbare Optionen auf. Eine Engine lässt sich direkt auswählen, etwa mit `ropero install engines basalt`. Das Modell wird separat installiert: `ropero install models <modell>`.

## Modelle und Nutzung

Manifeste in `engines/` beschreiben Engines und Installation; `models/<engine>/` enthält TOML-Definitionen der Modelle. Im normalen Betrieb liegt der aktive Katalog unter `~/.config/ropero/models/`. Die TOML-Dateien in diesem Repository sind Beispiele und werden nicht automatisch geladen.

```bash
ropero models add
ropero models add --path modell.toml
ropero <modell>
ropero status --live
```

Ausführliche Anleitungen gibt es auf [Spanisch](docs/engine-support.es.md) und [Englisch](docs/engine-support.en.md), in der [CLI-Referenz](docs/especificacion/ropero-cli.en.md) und im [Integrationsleitfaden](docs/agent-integrations.en.md). Der [deutsche Benutzerleitfaden](docs/guide.de.md) ist ebenfalls verfügbar. Benutzerleitfäden gibt es in zehn Sprachen; ausführliche Kompatibilitäts- und Integrationsreferenzen derzeit auf Englisch und Spanisch.
