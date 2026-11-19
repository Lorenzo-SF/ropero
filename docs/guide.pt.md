# Guia do Ropero

[English](guide.en.md) · [Español](guide.es.md) · **Português** · [Français](guide.fr.md) · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero é uma CLI em Rust para instalar, configurar e executar modelos de IA locais em diferentes engines. Ela prepara a engine declarada, inicia e monitora o serviço e oferece uma API compatível com OpenAI quando a engine permite.

## Começar

Ropero v2 ainda está em desenvolvimento. Na raiz do repositório:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

`ropero install engines` lista as engines instaláveis; também se pode selecionar uma diretamente, por exemplo `ropero install engines basalt`. A instalação da engine prepara o runtime e suas dependências. Instale o modelo separadamente com `ropero install models <modelo>`.

O CLI usa inglês por padrão. Para salvar a interface em espanhol, execute `ropero config --lang ES`; use `ropero config --lang EN` para voltar ao inglês.

## Catálogo e TOML

Os manifestos em `engines/` descrevem requisitos, detecção, instalação e capacidades. Os arquivos `models/<engine>/` definem o modelo, formato, download, artefatos e opções de execução. Em uma instalação normal, os modelos ativos ficam em `~/.config/ropero/models/<engine>/` e os manifestos próprios em `~/.config/ropero/engines/`. Os TOML do repositório são exemplos; não são carregados automaticamente.

```bash
ropero models add
ropero models add --path modelo.toml
ropero install models <modelo>
ropero <modelo>
ropero status --live
```

Para Basalt, `ropero install engines basalt` prepara o runtime; o pack é instalado separadamente. Basalt exige Linux x86-64, CPU AVX2, GPU NVIDIA Blackwell e driver compatível. O exemplo Qwen3.8-Flash-Next está em `models/basalt/`; downloads do Hugging Face podem exigir o CLI `hf` e autenticação.

## Engines e limites

- **llama-server** e **turbo-llama** servem modelos GGUF. Turbo-Llama é um fork; confirme as opções com a versão instalada.
- **FastFlowLM** delega operações ao CLI `flm`; suporte de formato e NPU depende de FastFlowLM e XRT.
- **Ignis** baixa artefatos próprios e os serve pelo diretório de artefatos configurado.
- **Basalt** serve packs `.basalt`. O Ropero oferece suporte apenas no Linux x86-64. Após instalar o engine, registra o TOML incluído do Qwen3.8-Flash-Next se ele ainda não existir; o pack é instalado separadamente.
- **Strata** serve perfis já preparados; Ropero não cria nem baixa perfis.
- **vLLM**, **MLX-Serve** e **TensorRT-LLM** têm adaptadores, mas não foram validados com um arranque real nesta máquina. MLX-Serve exige macOS e Apple Silicon.
- **AirLLM** permanece parcial: não há adaptador de servidor no Ropero.

O suporte varia por sistema, hardware, versão e modelo. Consulte [compatibilidade detalhada em inglês](engine-support.en.md) ou [espanhol](engine-support.es.md) antes de escolher.

## Agentes, IDE e ferramentas

`ropero connect --list` mostra os destinos automáticos. Os conectores implementados incluem OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw e VS Code. Execute primeiro o modelo no Ropero; a conexão configura o cliente, mas não instala o agente nem inicia a engine.

Zed aceita endpoints OpenAI-compatible e guarda chaves no chaveiro do sistema; configure-o manualmente em Agent Settings. A documentação oficial do Cursor não confirma conexão direta a endpoints locais do Ropero. MicroClaw pode usar `llm_base_url` em sua configuração; proteja o arquivo que contém tokens. Graphify é uma ferramenta opcional para enriquecer a análise do repositório através de um assistente compatível, não uma engine ou modelo do Ropero.

Consulte [referências detalhadas das integrações](agent-integrations.en.md) e as [opções do CLI](especificacion/ropero-cli.en.md).

## Ativar e desativar modelos

`ropero models` mostra por padrão os engines instalados e os modelos prontos para uso. Um engine instalado sem modelos disponíveis aparece com um aviso. Use `--all` para consultar todo o catálogo ou `--fired` para ver os modelos desativados. Despedir ou reativar move o TOML para dentro ou para fora da pasta `fired/` do engine; não apaga a definição nem os arquivos do modelo. Se o nome já estiver ativo, o Ropero pede um novo nome exclusivo antes de reativar.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
