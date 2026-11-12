#!/usr/bin/env python3
"""Conversor de pesos falso: el motor de verdad, en miniatura.

El gguffer lo llama asi:

    python3 convert_hf_to_gguf.py <dir_pesos> --outfile <salida> --outtype f16

El conversor de verdad, cuando el repo ya viene cuantizado, NO cuantiza:
escribe los tensores tal cual (los de NVFP4 en NVFP4) y pone el
general.file_type correspondiente. Este falso hace lo mismo, decided por la
metadata del repo:

    quant_method=compressed-tensors + format=nvfp4-pack-quantized -> 39 (NVFP4)
    quant_method=modelopt            + quant_algo=NVFP4            -> 39 (NVFP4)
    quant_method=mxfp4                                            -> 38 (MXFP4_MOE)
    hf_quant_config.json con quantization.quant_algo=NVFP4        -> 39 (NVFP4)
    resto (fp8, awq, gptq, bf16 normal)                           -> 1 (F16)

asi el test puede comprobar que el gguffer coloca el GGUF con el quant
que toca, y no con el que le da la gana.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import gguf_fake  # noqa: E402


def _carga(ruta):
    try:
        with open(ruta, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def file_type_del_repo(dir_pesos):
    """El llama_ftype que sale de convertir este repo."""
    cfg = _carga(os.path.join(dir_pesos, "config.json"))
    hqc = _carga(os.path.join(dir_pesos, "hf_quant_config.json"))

    qcfg = cfg.get("quantization_config") or {}
    metodo = qcfg.get("quant_method")
    formato = qcfg.get("format")
    algo = qcfg.get("quant_algo")
    grupos = qcfg.get("config_groups") or {}

    if metodo == "compressed-tensors":
        if formato == "nvfp4-pack-quantized":
            return 39
        if formato == "mixed-precision":
            # Precision mixta: hay un grupo nvfp4 y otro de otro tipo.
            for g in grupos.values():
                if isinstance(g, dict) and g.get("format") == "nvfp4-pack-quantized":
                    return 39
            return 1
        return 1
    if metodo == "modelopt":
        return 39 if algo == "NVFP4" else 1
    if metodo == "mxfp4":
        return 38

    hq = (hqc.get("quantization") or {})
    if str(hq.get("quant_algo") or "").upper() == "NVFP4":
        return 39
    if str(hq.get("quant_algo") or "").upper() == "MXFP4":
        return 38
    return 1


def main(argv):
    if len(argv) < 2:
        print("uso: convert_hf_to_gguf.py DIR --outfile F --outtype T", file=sys.stderr)
        return 2
    dir_pesos = argv[1]
    salida = None
    outtype = "f16"
    i = 2
    while i < len(argv):
        if argv[i] == "--outfile" and i + 1 < len(argv):
            salida = argv[i + 1]
            i += 2
            continue
        if argv[i] == "--outtype" and i + 1 < len(argv):
            outtype = argv[i + 1]
            i += 2
            continue
        i += 1
    if not salida:
        print("convert falso: falta --outfile", file=sys.stderr)
        return 2

    ft = file_type_del_repo(dir_pesos)
    if ft == 1 and str(outtype).lower() == "bf16":
        ft = 32

    # El repo base: el conversor de verdad lo guarda en el GGUF, y de ahi
    # sale el "base_model" de la procedencia.
    base_repo = ""
    base_name = ""
    cfg = _carga(os.path.join(dir_pesos, "config.json"))
    if cfg.get("_id"):
        base_repo = str(cfg.get("_id"))
    if cfg.get("_name"):
        base_name = str(cfg.get("_name"))

    gguf_fake.crear(salida, file_type=ft, arch="qwen35", block_count=5,
                    nextn=1, base_repo=base_repo, base_name=base_name)
    print("gguf: file type = %d" % ft)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
