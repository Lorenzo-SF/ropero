#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/_gptoss_common.sh"
# Sin alias público: este script solo es accesible como `gptoss_low` en ropero.
MODEL_ALIAS="${MODEL_ALIAS:-}"

get_model_args_gptoss_low() {
    get_model_args_gptoss_common low
}