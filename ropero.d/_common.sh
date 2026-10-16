#!/usr/bin/env bash
# =============================================================================
# _common.sh — colorize y utilidades para ropero
# =============================================================================

colorize() {
    local theme="${1:-default}" engine="${2:-llama-server}"

    awk -v t="$theme" -v e="$engine" '
    BEGIN {
        RST = "\033[0m"
        C1 = "\033[38;5;208m"
        C2 = "\033[38;5;214m"
        C3 = "\033[38;5;36m"
        C4 = "\033[38;5;51m"
        C5 = "\033[38;5;105m"
        C6 = "\033[38;5;201m"
        C7 = "\033[38;5;141m"
        C8 = "\033[38;5;228m"
        DEF = "\033[1;32m"
        BRED = "\033[1;31m"
        YEL = "\033[1;33m"
        MAG = "\033[1;35m"
        CYN = "\033[2;36m"
        GRY = "\033[90m"

        if (t == "gptoss") MAIN = C1
        else if (t == "qwen") MAIN = C2
        else if (t == "coder") MAIN = C3
        else if (t == "coder30b") MAIN = C4
        else if (t == "codernext") MAIN = C5
        else if (t == "coder30a3b") MAIN = C5
        else if (t == "embed") MAIN = C6
        else if (t == "muse") MAIN = C7
        else if (t == "glmf") MAIN = C8
        else if (t ~ /air/) MAIN = C7
        else if (e == "tensorrt-llm") MAIN = C6
        else if (e == "mlx_lm") MAIN = C5
        else MAIN = DEF
    }
    function c(color, msg) { printf "%s%s%s\n", color, msg, RST }
    {
        line = $0
        lower = tolower(line)
        if (lower ~ /(oom|out of memory|cuda error|metal.*error|segfault|abort|fatal|traceback|error|[[:space:]]5[0-9][0-9]([[:space:]]|$))/)
            { c(BRED, line); next }
        if (lower ~ /(tok\/s|tokens per second|throughput|eval time|prompt eval)/)
            { c(MAG, line); next }
        if (e == "tensorrt-llm" && line ~ /^\[TRT-LLM\][[:space:]]*[IWE]/)
            { c(MAIN, line); next }
        if (e == "airllm" && line ~ /(uvicorn|FastAPI|airllm-server|Uvicorn running|Application startup|HTTP\/)/)
            { c(MAIN, line); next }
        if (e == "mlx_lm" && line ~ /(mlx|throughput|tokens\/s|memory|HTTP\/)/)
            { c(MAIN, line); next }
        if (e == "fastflowlm" && line ~ /(flm|FastFlowLM|serve|listening)/)
            { c(MAIN, line); next }
        if (line ~ /(authproxy|proxy.*(listening|started)|loaded|llm_load|CUDA|Metal|GPU|server listening|ready)/)
            { c(MAIN, line); next }
        if (line ~ /(WRN|warning|deprecated)/)
            { c(YEL, line); next }
        if (line ~ /(HTTP|GET|POST|chat|srv)/)
            { c(CYN, line); next }
        c(GRY, line)
    }'
}
