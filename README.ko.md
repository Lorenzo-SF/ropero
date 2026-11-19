![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**AI 모델을 로컬에 설치하고 실행하세요.**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · **한국어** · [Русский](README.ru.md)

Ropero는 로컬 모델과 모델을 실행하는 엔진을 관리합니다. 모델을 선택하면 선언된 엔진을 준비하고 서비스를 시작한 뒤 준비 상태를 확인합니다. 엔진이 지원하면 OpenAI 호환 API도 제공합니다.

## 주요 기능

- 엔진(`ropero install engines`)과 모델(`ropero install models`)을 별도로 설치합니다.
- 요구 사항, 다운로드, 실행 옵션을 TOML 카탈로그로 선언합니다.
- 모델 서비스를 시작하고 상태를 확인하거나 종료합니다.
- 진단, 설정 정보, 실행 명령, 평가 도구를 제공합니다.
- 호환되는 코딩 에이전트와 연결하며 각 통합의 제한 사항을 안내합니다.

## 시작하기

이 저장소에는 아직 개발 중인 Ropero v2가 들어 있습니다. 저장소 루트에서 빌드하세요.

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

엔진 설치 명령은 선택 가능한 엔진을 표시합니다. 예를 들어 `ropero install engines basalt`로 하나를 선택할 수 있습니다. 모델은 `ropero install models <모델>`로 별도 설치합니다.

## 모델과 사용법

`engines/`의 매니페스트는 엔진과 설치 방법을 설명하고, [`models/<엔진>/`](models/)에는 모델 TOML 정의가 있습니다. 일반적인 사용에서 활성 카탈로그는 `~/.config/ropero/models/`에 있습니다. 저장소의 TOML 파일은 예제이며 자동으로 로드되지 않습니다.

```bash
ropero models add
ropero models add --path models/llama-server/groot-o.toml
ropero <모델>
ropero status --live
```

자세한 내용은 [스페인어](docs/engine-support.es.md) 및 [영어](docs/engine-support.en.md) 가이드, [CLI 참조](docs/especificacion/ropero-cli.en.md), [통합 가이드](docs/agent-integrations.en.md)를 참고하세요. [한국어 사용자 가이드](docs/guide.ko.md)도 참고하세요. 사용자 가이드는 10개 언어로 제공하며, 호환성 및 통합 상세 참조는 현재 영어와 스페인어로 제공됩니다.
