# Guida a Ropero

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · [Deutsch](guide.de.md) · **Italiano** · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero è una CLI Rust per installare, configurare ed eseguire modelli IA locali con diversi engine. Prepara l’engine dichiarato, avvia e monitora il servizio e offre un’API compatibile con OpenAI quando l’engine la supporta.

## Per iniziare

Ropero v2 è ancora in sviluppo. Dalla radice del repository:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

`ropero install engines` elenca gli engine installabili; puoi sceglierne uno direttamente, ad esempio `ropero install engines basalt`. Il comando prepara il runtime e le dipendenze dell’engine. Installa il modello separatamente con `ropero install models <modello>`.

La lingua iniziale del CLI è l’inglese. Per salvare l’interfaccia in spagnolo usa `ropero config --lang ES`; `ropero config --lang EN` ripristina l’inglese.

## Catalogo e TOML

I manifesti in `engines/` descrivono requisiti, rilevamento, installazione e capacità. I file in `models/<engine>/` definiscono modello, formato, download, artefatti e parametri di esecuzione. Nell’uso normale, i modelli attivi si trovano in `~/.config/ropero/models/<engine>/` e i manifesti personalizzati in `~/.config/ropero/engines/`. I TOML nel repository sono esempi e non vengono caricati automaticamente.

```bash
ropero models add
ropero models add --path modello.toml
ropero install models <modello>
ropero <modello>
ropero status --live
```

Per Basalt, `ropero install engines basalt` prepara il runtime; il pack del modello si installa a parte. Basalt richiede Linux x86-64, CPU AVX2, GPU NVIDIA Blackwell e un driver compatibile. L’esempio Qwen3.8-Flash-Next si trova in `models/basalt/`; i download Hugging Face possono richiedere la CLI `hf` e l’autenticazione.

## Engine e limiti

- **llama-server** e **turbo-llama** servono modelli GGUF. Turbo-Llama è un fork: verifica le opzioni della versione installata.
- **FastFlowLM** delega le operazioni alla CLI `flm`; formati e supporto NPU dipendono da FastFlowLM e XRT.
- **Ignis** scarica artefatti propri e li serve dalla directory configurata.
- **Basalt** serve pack `.basalt`. Ropero lo supporta solo su Linux x86-64. Dopo l’installazione dell’engine registra il TOML Qwen3.8-Flash-Next incluso, se manca; il pack del modello si installa separatamente.
- **Strata** serve profili già preparati; Ropero non crea né scarica profili.
- **vLLM**, **MLX-Serve** e **TensorRT-LLM** hanno adapter, ma su questa macchina non è stato verificato un avvio reale. MLX-Serve richiede macOS e Apple Silicon.
- **AirLLM** è parziale: Ropero non dispone di un adapter server per questo engine.

Il supporto dipende da sistema operativo, hardware, versione e modello. Consulta le guide dettagliate di [compatibilità in inglese](engine-support.en.md) o [spagnolo](engine-support.es.md).

## Agenti, IDE e strumenti

`ropero connect --list` mostra le integrazioni automatiche: OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw e VS Code. Avvia prima il modello con Ropero; la connessione configura il client, ma non installa l’agente né avvia l’engine.

Zed supporta endpoint OpenAI-compatibili e conserva le chiavi nel portachiavi di sistema; configurarlo in Agent Settings. La documentazione ufficiale di Cursor non conferma la connessione diretta agli endpoint locali Ropero. MicroClaw può usare `llm_base_url`; proteggi il file di configurazione che contiene token. Graphify è uno strumento facoltativo per arricchire l’analisi del repository con un assistente compatibile; non è un engine o un modello Ropero.

Consulta la [guida dettagliata alle integrazioni](agent-integrations.en.md) e il [riferimento CLI](especificacion/ropero-cli.en.md).

## Attivare e disattivare i modelli

`ropero models` mostra per impostazione predefinita gli engine installati e i modelli pronti all’uso. Un engine installato senza modelli disponibili viene mostrato con un avviso. Usa `--all` per consultare l’intero catalogo oppure `--fired` per vedere i modelli disattivati. Disattivare o riattivare sposta il TOML dentro o fuori dalla cartella `fired/` dell’engine; non elimina la definizione né i file del modello. Se il nome è già attivo, Ropero chiede un nuovo nome univoco prima di riattivarlo.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
