#!/usr/bin/env python3
"""GGUF sintetico para los fakes del gguffer.

Es lo que el conversor y el cuantizador falsos escriben: un GGUF de
cabecera valida (lo que lee ggufcheck.py) con el general.file_type que
cada fake necesita.

Modos (CLI):
    create <fichero> <file_type> [arch] [block_count]
    file_type <fichero>
    requant <entrada> <salida> <QUANT>
    set_file_type <fichero> <file_type>

El numero de file_type es un llama_ftype (el enum de include/llama.h del
motor): 39=NVFP4, 38=MXFP4_MOE, 32=BF16, 15=Q4_K_M, 1=F16. Es el MISMO
numero que lleva general.file_type en un GGUF de verdad.
"""

import os
import struct
import sys

# Los tipos de quant en el espacio del MOTOR (llama_ftype). Los numeros
# son los de include/llama.h del motor: 39=NVFP4, 38=MXFP4_MOE, 32=BF16,
# 15=Q4_K_M. El GGUF los lleva en general.file_type.
FTYPES = {
    "F32": 0,
    "F16": 1,
    "Q4_0": 2,
    "Q4_1": 3,
    "Q8_0": 7,
    "Q5_0": 8,
    "Q5_1": 9,
    "Q2_K": 10,
    "Q3_K_S": 11,
    "Q3_K_M": 12,
    "Q3_K_L": 13,
    "Q4_K_S": 14,
    "Q4_K_M": 15,
    "Q5_K_S": 16,
    "Q5_K_M": 17,
    "Q6_K": 18,
    "IQ2_XXS": 19,
    "IQ2_XS": 20,
    "Q2_K_S": 21,
    "IQ3_XS": 22,
    "IQ3_XXS": 23,
    "IQ1_S": 24,
    "IQ4_NL": 25,
    "IQ3_S": 26,
    "IQ3_M": 27,
    "IQ2_S": 28,
    "IQ2_M": 29,
    "IQ4_XS": 30,
    "IQ1_M": 31,
    "BF16": 32,
    "TQ1_0": 36,
    "TQ2_0": 37,
    "MXFP4_MOE": 38,
    "NVFP4": 39,
}


def _s(x):
    b = x.encode("utf-8")
    return struct.pack("<Q", len(b)) + b


def _kv(key, vtype, val):
    return _s(key) + struct.pack("<I", vtype) + val


def _vsize(vtype, data, pos):
    """Cuantos bytes ocupa el valor de una KV que empieza en pos."""
    if vtype == 8:  # string
        n = struct.unpack_from("<Q", data, pos)[0]
        return 8 + n
    if vtype == 4:  # uint32
        return 4
    if vtype == 10:  # uint64
        return 8
    raise ValueError("tipo de valor no soportado en el fake: %r" % vtype)


def crear(path, file_type=1, arch="qwen35", block_count=5, nextn=1,
          base_repo=None, base_name=None, tensores=None):
    """Escribe un GGUF de cabecera valida.

    tensores: lista de nombres; cada uno es un tensor F32 de 8 elementos.
    """
    if tensores is None:
        tensores = ["blk.0.w", "blk.4.w"]

    kvs = [
        _kv("general.architecture", 8, _s(arch)),
        _kv("general.file_type", 4, struct.pack("<I", file_type)),
    ]
    if base_repo:
        kvs.append(_kv("general.base_model.0.repo_url", 8, _s(base_repo)))
    if base_name:
        kvs.append(_kv("general.base_model.0.name", 8, _s(base_name)))
    kvs.append(_kv("%s.block_count" % arch, 4, struct.pack("<I", block_count)))
    kvs.append(_kv("%s.nextn_predict_layers" % arch, 4, struct.pack("<I", nextn)))

    out = b"GGUF" + struct.pack("<IQQ", 3, len(tensores), len(kvs))
    for k in kvs:
        out += k

    offset = 0
    for nombre in tensores:
        out += _s(nombre) + struct.pack("<IQIQ", 1, 8, 0, offset)
        offset += 32

    out += b"\0" * ((-len(out)) % 32)
    out += b"\0" * (offset + 64)

    with open(path, "wb") as f:
        f.write(out)
    return path


def set_ftype(path, file_type):
    """Cambia el general.file_type de un GGUF ya escrito, en su sitio."""
    with open(path, "r+b") as f:
        data = bytearray(f.read())

    n_kv = struct.unpack_from("<Q", data, 16)[0]
    pos = 24
    for _ in range(n_kv):
        klen = struct.unpack_from("<Q", data, pos)[0]
        pos += 8
        key = bytes(data[pos:pos + klen]).decode("utf-8")
        pos += klen
        vtype = struct.unpack_from("<I", data, pos)[0]
        pos += 4
        if key == "general.file_type":
            struct.pack_into("<I", data, pos, file_type)
            with open(path, "wb") as f:
                f.write(data)
            return
        pos += _vsize(vtype, data, pos)
    raise ValueError("el GGUF no tiene general.file_type: %s" % path)


def get_ftype(path):
    """El general.file_type de un GGUF (o None si no lo tiene)."""
    with open(path, "rb") as f:
        data = f.read()
        if len(data) < 24 or data[:4] != b"GGUF":
            return None
        n_kv = struct.unpack_from("<Q", data, 16)[0]
        pos = 24
        for _ in range(n_kv):
            if pos + 8 > len(data):
                break
            klen = struct.unpack_from("<Q", data, pos)[0]
            pos += 8
            key = bytes(data[pos:pos + klen]).decode("utf-8", "replace")
            pos += klen
            vtype = struct.unpack_from("<I", data, pos)[0]
            pos += 4
            if key == "general.file_type":
                return struct.unpack_from("<I", data, pos)[0]
            pos += _vsize(vtype, data, pos)
    return None


def main(argv):
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    cmd = argv[1]
    if cmd == "create" and len(argv) >= 4:
        ft = int(argv[3])
        arch = argv[4] if len(argv) > 4 else "qwen35"
        crear(argv[2], file_type=ft, arch=arch)
        return 0
    if cmd == "file_type" and len(argv) >= 3:
        ft = get_ftype(argv[2])
        if ft is None:
            return 1
        print(ft)
        return 0
    if cmd == "set_file_type" and len(argv) >= 4:
        set_ftype(argv[2], int(argv[3]))
        return 0
    if cmd == "requant" and len(argv) >= 5:
        entrada, salida, quant = argv[2], argv[3], argv[4]
        ft = FTYPES.get(quant)
        if ft is None:
            print("gguf_fake: quant desconocido: %s" % quant, file=sys.stderr)
            return 2
        with open(entrada, "rb") as f:
            data = f.read()
        with open(salida, "wb") as f:
            f.write(data)
        set_ftype(salida, ft)
        return 0
    print("gguf_fake: modo desconocido: %s" % cmd, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
