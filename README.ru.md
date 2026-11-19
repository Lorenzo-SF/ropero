![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**Устанавливайте и запускайте модели ИИ локально.**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · **Русский**

Ropero управляет локальными моделями и движками, которые их запускают. Выберите модель: Ropero подготовит указанный движок, запустит службу, проверит её готовность и предоставит API, совместимый с OpenAI, если движок его поддерживает.

## Возможности

- Раздельная установка движков (`ropero install engines`) и моделей (`ropero install models`).
- Декларативный каталог TOML с требованиями, загрузками и параметрами запуска.
- Запуск, контроль состояния и остановка модельных служб.
- Диагностика, сведения о конфигурации, команды запуска и оценка моделей.
- Интеграции с совместимыми помощниками для программирования с описанием ограничений.

## Начало работы

В репозитории находится Ropero v2, который всё ещё разрабатывается. Соберите его из корня проекта:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

Команда установки движков показывает доступные варианты. Можно указать движок напрямую, например `ropero install engines basalt`. Модель устанавливается отдельно: `ropero install models <модель>`.

## Модели и использование

Манифесты в `engines/` описывают движки и их установку; в [`models/<движок>/`](models/) находятся TOML-описания моделей. При обычном использовании активный каталог хранится в `~/.config/ropero/models/`. TOML-файлы репозитория — это примеры, они не загружаются автоматически.

```bash
ropero models add
ropero models add --path models/llama-server/groot-o.toml
ropero <модель>
ropero status --live
```

Подробности см. в руководствах на [испанском](docs/engine-support.es.md) и [английском](docs/engine-support.en.md), в [справочнике CLI](docs/especificacion/ropero-cli.en.md) и [руководстве по интеграциям](docs/agent-integrations.en.md). Также доступно [руководство пользователя на русском](docs/guide.ru.md). Руководства пользователя есть на десяти языках; подробные справочники по совместимости и интеграциям пока доступны на английском и испанском.
