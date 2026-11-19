![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**Installa ed esegui modelli IA in locale.**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · **Italiano** · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · [Русский](README.ru.md)

Ropero organizza i modelli locali e i motori che li eseguono. Scegli un modello: Ropero prepara il motore dichiarato, avvia il servizio, ne verifica la disponibilità e offre un’API compatibile con OpenAI quando il motore la supporta.

## Funzionalità

- Installazione separata dei motori (`ropero install engines`) e dei modelli (`ropero install models`).
- Catalogo dichiarativo TOML per requisiti, download e parametri di esecuzione.
- Avvio, monitoraggio e arresto dei servizi dei modelli.
- Diagnostica, informazioni di configurazione, comandi di avvio e valutazioni.
- Integrazioni con assistenti di programmazione compatibili, con limiti documentati.

## Per iniziare

Questo repository contiene Ropero v2, ancora in sviluppo. Compila dalla directory principale:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

Il comando degli engine elenca quelli disponibili; puoi sceglierne uno, ad esempio `ropero install engines basalt`. Installa il modello separatamente con `ropero install models <modello>`.

## Modelli e utilizzo

I manifesti in `engines/` descrivono gli engine e l’installazione; [`models/<engine>/`](models/) contiene definizioni TOML dei modelli. Nell’uso normale il catalogo attivo si trova in `~/.config/ropero/models/`. I file TOML del repository sono esempi e non vengono caricati automaticamente.

```bash
ropero models add
ropero models add --path models/llama-server/groot-o.toml
ropero <modello>
ropero status --live
```

Consulta le guide dettagliate in [spagnolo](docs/engine-support.es.md) e [inglese](docs/engine-support.en.md), la [guida CLI](docs/especificacion/ropero-cli.en.md) e la [guida alle integrazioni](docs/agent-integrations.en.md). Consulta anche la [guida utente in italiano](docs/guide.it.md). Le guide utente sono disponibili in dieci lingue; i riferimenti dettagliati su compatibilità e integrazioni sono attualmente in inglese e spagnolo.
