#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/_gptoss_common.sh"
# Alias público (lo carga bin/ropero como nombre alternativo del modelo).
# Para cambiar el modelo físico asociado al alias `verifier`, edita este script;
# `opencode.jsonc` no se entera.
MODEL_ALIAS="${MODEL_ALIAS:-verifier}"

get_model_args_gptoss_high() {
    get_model_args_gptoss_common high
}