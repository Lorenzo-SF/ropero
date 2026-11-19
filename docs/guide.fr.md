# Guide Ropero

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · **Français** · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero est une CLI Rust pour installer, configurer et exécuter des modèles d’IA locaux avec différents moteurs. Elle prépare le moteur déclaré, démarre et surveille le service et fournit une API compatible avec OpenAI lorsque le moteur le permet.

## Démarrer

Ropero v2 est encore en développement. Depuis la racine du dépôt :

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

`ropero install engines` liste les moteurs installables ; vous pouvez aussi en choisir un directement, par exemple `ropero install engines basalt`. L’installation du moteur prépare son runtime et ses dépendances. Installez le modèle séparément avec `ropero install models <modèle>`.

La langue du CLI est l’anglais par défaut. Pour enregistrer l’interface en espagnol, lancez `ropero config --lang ES` ; `ropero config --lang EN` rétablit l’anglais.

## Catalogue et TOML

Les manifestes de `engines/` décrivent les prérequis, la détection, l’installation et les capacités. Les fichiers `models/<engine>/` définissent le modèle, son format, son téléchargement, ses artefacts et ses options d’exécution. Dans une installation normale, les modèles actifs se trouvent dans `~/.config/ropero/models/<engine>/` et les manifestes personnels dans `~/.config/ropero/engines/`. Les TOML du dépôt sont des exemples et ne sont pas chargés automatiquement.

```bash
ropero models add
ropero models add --path modele.toml
ropero install models <modèle>
ropero <modèle>
ropero status --live
```

Pour Basalt, `ropero install engines basalt` prépare le runtime ; le pack du modèle s’installe séparément. Basalt requiert Linux x86-64, un processeur AVX2, un GPU NVIDIA Blackwell et un pilote compatible. L’exemple Qwen3.8-Flash-Next se trouve dans `models/basalt/` ; les téléchargements Hugging Face peuvent nécessiter la CLI `hf` et une authentification.

## Moteurs et limites

- **llama-server** et **turbo-llama** servent des modèles GGUF. Turbo-Llama est un fork : vérifiez les options de la version installée.
- **FastFlowLM** délègue les opérations à la CLI `flm` ; formats et NPU dépendent de FastFlowLM et XRT.
- **Ignis** télécharge ses artefacts et les sert depuis le répertoire configuré.
- **Basalt** sert des packs `.basalt`. Ropero le prend en charge uniquement sous Linux x86-64. Après l’installation du moteur, il enregistre le TOML Qwen3.8-Flash-Next fourni s’il est absent ; le pack reste à installer séparément.
- **Strata** sert des profils préparés ; Ropero ne crée ni ne télécharge les profils.
- **vLLM**, **MLX-Serve** et **TensorRT-LLM** ont des adaptateurs, mais aucun démarrage réel n’a été validé sur cette machine. MLX-Serve nécessite macOS et Apple Silicon.
- **AirLLM** reste partiel : Ropero ne possède pas d’adaptateur de serveur pour ce moteur.

La prise en charge dépend du système, du matériel, de la version et du modèle. Consultez les guides de [compatibilité détaillé en anglais](engine-support.en.md) ou en [espagnol](engine-support.es.md).

## Agents, IDE et outils

`ropero connect --list` affiche les intégrations automatiques. Elles comprennent OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw et VS Code. Lancez d’abord le modèle avec Ropero ; la connexion configure le client, mais n’installe pas l’agent et ne démarre pas le moteur.

Zed accepte les endpoints OpenAI compatibles et stocke les clés dans le trousseau système ; configurez-le dans Agent Settings. La documentation officielle de Cursor ne confirme pas la connexion directe aux endpoints locaux Ropero. MicroClaw accepte `llm_base_url` dans sa configuration ; protégez le fichier qui contient les jetons. Graphify est un outil facultatif qui enrichit l’analyse d’un dépôt via un assistant compatible ; ce n’est ni un moteur ni un modèle Ropero.

Voir les [détails des intégrations](agent-integrations.en.md) et la [référence CLI](especificacion/ropero-cli.en.md).

## Activer et désactiver des modèles

`ropero models` affiche par défaut les moteurs installés et les modèles prêts à l’emploi. Un moteur installé sans modèle disponible reste affiché avec un message. Utilisez `--all` pour consulter tout le catalogue ou `--fired` pour voir les modèles désactivés. Désactiver ou réactiver déplace le TOML vers ou depuis le dossier `fired/` du moteur ; cela ne supprime ni la définition ni les fichiers du modèle. Si le nom est déjà actif, Ropero demande un nouveau nom unique avant la réactivation.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
