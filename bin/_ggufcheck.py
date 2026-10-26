#!/usr/bin/env python3
"""
Lee y valida un GGUF. Lo usa 'ropero gguffer' para no borrar los
safetensors cuando el GGUF final no esta bien.

  _ggufcheck.py FICHERO              -> valida (exit 0 ok, 1 mal)
  _ggufcheck.py FICHERO --info       -> imprime la metadata y sale

El tamano de los tensores NO esta escrito aqui: se pregunta a la propia
libggml (ctypes), que es la fuente de verdad. Si no se puede cargar, el
check de tamano se salta con un aviso y el resto del check sigue valiendo.
"""

import ctypes
import json
import os
import struct
import sys

# ── La metadata del GGUF ────────────────────────────────────────────────────
# (de ggml/src/gguf.cpp): el string es uint64 len + bytes, el valor depende
# del tipo.
GGUF_TYPE_UINT8   = 0
GGUF_TYPE_INT8    = 1
GGUF_TYPE_UINT16  = 2
GGUF_TYPE_INT16   = 3
GGUF_TYPE_UINT32  = 4
GGUF_TYPE_INT32   = 5
GGUF_TYPE_FLOAT32 = 6
GGUF_TYPE_BOOL    = 7
GGUF_TYPE_STRING  = 8
GGUF_TYPE_ARRAY   = 9
GGUF_TYPE_UINT64  = 10
GGUF_TYPE_INT64   = 11
GGUF_TYPE_FLOAT64 = 12

# (formato de struct, bytes). El endianness lo pone el "<" de quien lee.
ESCALARES = {
    GGUF_TYPE_UINT8:  ("B", 1), GGUF_TYPE_INT8:  ("b", 1),
    GGUF_TYPE_UINT16: ("H", 2), GGUF_TYPE_INT16: ("h", 2),
    GGUF_TYPE_UINT32: ("I", 4), GGUF_TYPE_INT32: ("i", 4),
    GGUF_TYPE_FLOAT32:("f", 4), GGUF_TYPE_BOOL:("B", 1),
    GGUF_TYPE_UINT64: ("Q", 8), GGUF_TYPE_INT64: ("q", 8),
    GGUF_TYPE_FLOAT64:("d", 8),
}


class GgufError(Exception):
    pass


def _cargar_libggml(rutas):
    """Carga libggml para preguntar los tamanos de los tipos. None si no."""
    for ruta in rutas:
        if not ruta or not os.path.exists(ruta):
            continue
        try:
            lib = ctypes.CDLL(ruta)
            lib.ggml_type_size.restype = ctypes.c_size_t
            lib.ggml_type_size.argtypes = [ctypes.c_int]
            lib.ggml_blck_size.restype = ctypes.c_int
            lib.ggml_blck_size.argtypes = [ctypes.c_int]
            return lib
        except OSError:
            continue
    return None


class Gguf:
    def __init__(self, path, lib_rutas=()):
        self.path = path
        self.tam = os.path.getsize(path)
        with open(path, "rb") as f:
            self.data = f.read()
        d = self.data
        if d[:4] != b"GGUF":
            raise GgufError("no empieza por GGUF (magico: %r)" % d[:4])
        off = 4
        (self.version,) = struct.unpack_from("<I", d, off); off += 4
        (self.n_tensors,) = struct.unpack_from("<Q", d, off); off += 8
        (self.n_kv,) = struct.unpack_from("<Q", d, off); off += 8
        self.kv = {}
        self.tensors = []
        self._leer_kv(off)
        self._leer_tensores()
        self.lib = _cargar_libggml(lib_rutas)

    def _str(self, off):
        (l,) = struct.unpack_from("<Q", self.data, off); off += 8
        if l > 1 << 30:
            raise GgufError("longitud de string imposible: %d" % l)
        s = self.data[off:off + l].decode("utf-8", "replace")
        return s, off + l

    def _leer_kv(self, off):
        d = self.data
        for _ in range(self.n_kv):
            key, off = self._str(off)
            (vt,) = struct.unpack_from("<I", d, off); off += 4
            if vt == GGUF_TYPE_STRING:
                val, off = self._str(off)
            elif vt == GGUF_TYPE_ARRAY:
                (et,) = struct.unpack_from("<I", d, off); off += 4
                (cnt,) = struct.unpack_from("<Q", d, off); off += 8
                if et == GGUF_TYPE_STRING:
                    val = []
                    for _ in range(cnt):
                        s, off = self._str(off); val.append(s)
                elif et in ESCALARES:
                    fmt, sz = ESCALARES[et]
                    val = list(struct.unpack_from("<" + fmt * cnt, d, off)); off += sz * cnt
                else:
                    raise GgufError("array de tipo %d (no soportado)" % et)
            elif vt in ESCALARES:
                fmt, sz = ESCALARES[vt]
                (val,) = struct.unpack_from("<" + fmt, d, off); off += sz
            else:
                raise GgufError("tipo de valor %d (no soportado) en la clave %s" % (vt, key))
            self.kv[key] = val
        self._offset_tras_kv = off

    def _leer_tensores(self):
        d = self.data
        # el indice de tensores va justo detras de la metadata: hay que
        # seguir la posicion actual, no el final del fichero.
        off = self._offset_tras_kv
        for _ in range(self.n_tensors):
            name, off = self._str(off)
            (ndim,) = struct.unpack_from("<I", d, off); off += 4
            dims = list(struct.unpack_from("<" + "Q" * ndim, d, off)); off += 8 * ndim
            (ttype,) = struct.unpack_from("<I", d, off); off += 4
            (toff,) = struct.unpack_from("<Q", d, off); off += 8
            self.tensors.append({"name": name, "dims": dims, "type": ttype, "offset": toff})

    # se rellena al terminar _leer_kv
    _offset_tras_kv = 0

    def tam_tensor(self, t):
        """Bytes que ocupa un tensor, o None si no se puede saber."""
        if self.lib is None:
            return None
        n = 1
        for x in t["dims"]:
            n *= x
        ts = self.lib.ggml_type_size(t["type"])
        bs = self.lib.ggml_blck_size(t["type"])
        if not ts or not bs:
            return None
        return (n // bs) * ts

    def resumen(self):
        arch = self.kv.get("general.architecture", "?")
        info = {
            "fichero": self.path,
            "bytes": self.tam,
            "version": self.version,
            "tensores": self.n_tensors,
            "arch": arch,
            "file_type": self.kv.get("general.file_type"),
            "size_label": self.kv.get("general.size_label"),
            "block_count": self.kv.get("%s.block_count" % arch),
            "nextn": self.kv.get("%s.nextn_predict_layers" % arch),
        }
        tipos = {}
        for t in self.tensors:
            tipos[t["type"]] = tipos.get(t["type"], 0) + 1
        info["tipos_de_tensor"] = dict(sorted(tipos.items()))
        return info


def main(argv):
    if len(argv) < 2:
        print("uso: _ggufcheck.py FICHERO [--info] [--lib RUTA]", file=sys.stderr)
        return 2
    path = argv[1]
    info_only = "--info" in argv
    libs = []
    for i, a in enumerate(argv):
        if a == "--lib" and i + 1 < len(argv):
            libs.append(argv[i + 1])
    if not os.path.exists(path):
        print("[x] no existe: %s" % path, file=sys.stderr)
        return 1
    try:
        g = Gguf(path, libs)
    except GgufError as e:
        print("[x] GGUF invalido: %s" % e, file=sys.stderr)
        return 1
    except (struct.error, IndexError) as e:
        print("[x] GGUF truncado o corrupto: %s" % e, file=sys.stderr)
        return 1

    r = g.resumen()
    if info_only:
        print(json.dumps(r, indent=2, default=str))
        return 0

    # --tipo-predominante: el tipo de tensor que mas hay. Es el tipo de
    # los pesos, que es lo que dice si el GGUF ya esta quantizado. No
    # sirve el 'general.file_type' porque el conversor no lo pone.
    if "--tipo-predominante" in argv:
        if not g.tensors:
            print(0)
            return 1
        conteo = {}
        for t in g.tensors:
            conteo[t["type"]] = conteo.get(t["type"], 0) + 1
        print(max(conteo, key=lambda k: conteo[k]))
        return 0

    print("GGUF: %s" % path)
    print("  %.2f GiB | arch=%s | tensores=%d | file_type=%s | %s | nextn=%s"
          % (g.tam / 1073741824.0, r["arch"], r["tensores"], r["file_type"],
             r["size_label"] or "", r["nextn"] if r["nextn"] is not None else "-"))

    # 1. el indice de tensores tiene que caber en el fichero
    max_off = 0
    for t in g.tensors:
        if t["offset"] + 1 > g.tam:
            print("[x] el tensor '%s' esta mas alla del final del fichero" % t["name"], file=sys.stderr)
            return 1
        max_off = max(max_off, t["offset"])

    # 2. el ultimo tensor tiene que caber entero
    if g.lib is None:
        print("[!] sin libggml: no puedo comprobar el tamano exacto de los datos", file=sys.stderr)
    else:
        ultimo = max(g.tensors, key=lambda t: t["offset"])
        tam = g.tam_tensor(ultimo)
        if tam is None:
            print("[!] tipo %d desconocido: no puedo comprobar el tamano" % ultimo["type"], file=sys.stderr)
        else:
            fin = ultimo["offset"] + tam
            if fin > g.tam:
                print("[x] el fichero esta truncado: el ultimo tensor acaba en %d y el fichero mide %d"
                      % (fin, g.tam), file=sys.stderr)
                return 1
            # el final debe ser el final del fichero (con algo de margen por
            # la alineacion)
            if g.tam - fin > 64:
                print("[!] sobran %d bytes al final (puede haber datos de mas, no es un error)"
                      % (g.tam - fin), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
