# Ropero 사용자 가이드

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · **한국어** · [Русский](guide.ru.md)

Ropero는 다양한 엔진으로 로컬 AI 모델을 설치, 설정, 실행하는 Rust CLI입니다. 선택한 엔진과 종속성을 준비하고 서비스를 시작해 모니터링합니다. 엔진이 지원하면 OpenAI 호환 API를 제공합니다. Ropero v2는 개발 중입니다.

## 시작하기

저장소 루트에서 CLI를 빌드하고 명령을 확인하세요.

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

engine 명령은 사용 가능한 엔진을 나열합니다. `ropero install engines basalt`처럼 직접 지정할 수도 있습니다. 엔진 설치는 런타임과 종속성을 준비합니다. 모델은 `ropero install models <model>`로 별도 설치하세요.

CLI 기본 언어는 영어입니다. 스페인어 설정을 저장하려면 `ropero config --lang ES`, 영어로 되돌리려면 `ropero config --lang EN`을 실행하세요.

## 카탈로그와 TOML

`engines/`의 매니페스트는 엔진 요구 사항, 감지, 설치 및 기능을 설명합니다. `models/<engine>/` 파일은 모델, 형식, 다운로드, 산출물 및 실행 옵션을 정의합니다. 일반적인 사용에서 활성 모델 정의는 `~/.config/ropero/models/<engine>/`, 사용자 엔진 매니페스트는 `~/.config/ropero/engines/`에 있습니다. 저장소의 TOML 파일은 예시이며 자동으로 로드되지 않습니다.

```bash
ropero models add
ropero models add --path model.toml
ropero install models <model>
ropero <model>
ropero status --live
```

## 엔진과 제한 사항

- **llama-server**와 **turbo-llama**는 GGUF 모델을 제공합니다.
- **FastFlowLM**은 `flm` CLI에 작업을 위임합니다. 형식 및 NPU 지원은 FastFlowLM과 XRT에 따라 다릅니다.
- **Ignis**는 다운로드한 엔진 산출물을 실행합니다.
- **Basalt**는 `.basalt` 팩을 실행합니다. Ropero는 Linux x86-64에서만 Basalt를 지원합니다. 엔진 설치가 성공하면 해당 파일이 없을 때 Qwen3.8-Flash-Next TOML을 사용자 카탈로그에 등록합니다. 모델 팩은 별도로 설치해야 합니다. AVX2 CPU, NVIDIA Blackwell GPU 및 호환 드라이버가 필요합니다.
- **Strata**는 준비된 프로필을 실행합니다. Ropero는 프로필을 만들거나 다운로드하지 않습니다.
- **vLLM**, **MLX-Serve**, **TensorRT-LLM**에는 어댑터가 있지만 지원 여부는 플랫폼과 모델에 따라 다릅니다. MLX-Serve에는 macOS와 Apple Silicon이 필요합니다.
- **AirLLM** 지원은 일부 기능에 한정되며 Ropero 서버 어댑터는 없습니다.

요구 사항과 알려진 제한 사항은 [엔진 호환성 상세 문서(영어)](engine-support.en.md)를 참조하세요.

## 에이전트, IDE 및 도구

`ropero connect --list`에서 OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw, VS Code 등의 자동 연결을 확인할 수 있습니다. 연결하기 전에 Ropero로 모델을 실행하세요. 연결 기능은 클라이언트를 설정하며 에이전트를 설치하거나 엔진을 시작하지 않습니다.

Zed는 OpenAI 호환 엔드포인트를 지원하고 키를 시스템 키체인에 저장합니다. Agent Settings에서 설정하세요. Cursor 공식 문서는 Ropero 로컬 엔드포인트에 대한 직접 연결을 확인하지 않습니다. MicroClaw는 설정의 `llm_base_url`을 사용할 수 있습니다. Graphify는 호환 어시스턴트와 함께 사용하는 선택적 저장소 분석 도구이며 Ropero 엔진이나 모델이 아닙니다.

[통합 참조(영어)](agent-integrations.en.md)와 [CLI 참조(영어)](especificacion/ropero-cli.en.md)도 확인하세요.

## 모델 활성화 및 비활성화

`ropero models`는 기본적으로 설치된 엔진과 바로 사용할 수 있는 모델을 표시합니다. 사용 가능한 모델이 없는 설치된 엔진도 안내와 함께 표시됩니다. `--all`은 전체 카탈로그를, `--fired`는 비활성화된 모델을 보여 줍니다. 비활성화와 재활성화는 TOML을 엔진의 `fired/` 폴더 안팎으로 이동하며 모델 정의나 파일을 삭제하지 않습니다. 이미 활성 모델이 같은 이름을 사용 중이면 재활성화 전에 고유한 새 이름을 입력하라는 메시지가 표시됩니다.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
