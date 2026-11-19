![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**Installez et exécutez des modèles d’IA localement.**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · **Français** · [Deutsch](README.de.md) · [Italiano](README.it.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · [Русский](README.ru.md)

Ropero organise les modèles locaux et les moteurs qui les exécutent. Choisissez un modèle : Ropero prépare le moteur déclaré, démarre le service, vérifie qu’il répond et fournit une API compatible avec OpenAI lorsque le moteur le permet.

## Fonctionnalités

- Installation séparée des moteurs (`ropero install engines`) et des modèles (`ropero install models`).
- Catalogue déclaratif TOML pour les prérequis, téléchargements et paramètres d’exécution.
- Démarrage, suivi et arrêt des services de modèles.
- Diagnostics, informations de configuration, commandes de lancement et évaluations.
- Intégrations avec les assistants de code compatibles, chacune avec ses propres limites.

## Démarrage

Ce dépôt contient Ropero v2, encore en développement. Compilez depuis la racine :

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

La commande des moteurs affiche les options disponibles. Vous pouvez en sélectionner un, par exemple `ropero install engines basalt`. Installez ensuite le modèle séparément avec `ropero install models <modèle>`.

## Modèles et utilisation

Les manifestes de `engines/` décrivent les moteurs et leur installation ; `models/<moteur>/` contient les définitions TOML des modèles. En usage normal, le catalogue se trouve dans `~/.config/ropero/models/`. Les fichiers TOML de ce dépôt sont des exemples et ne sont pas chargés automatiquement.

```bash
ropero models add
ropero models add --path modele.toml
ropero <modèle>
ropero status --live
```

Consultez les guides détaillés en [espagnol](docs/engine-support.es.md) et en [anglais](docs/engine-support.en.md), la [référence CLI](docs/especificacion/ropero-cli.en.md) et le [guide des intégrations](docs/agent-integrations.en.md). Consultez aussi le [guide utilisateur en français](docs/guide.fr.md). Les guides utilisateur existent en dix langues ; les références détaillées de compatibilité et d’intégration sont actuellement en anglais et en espagnol.
