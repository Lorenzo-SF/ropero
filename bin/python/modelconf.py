#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
_modelconf.py — cargador de modelos de ropero.

Un modelo de ropero es DATO, no código: vive en un .toml bajo `models/` y este
script es el único que sabe leerlos. No ejecuta nada de lo que lee (ni `exec`,
ni shell): sólo traduce tablas a variables y listas de argumentos.

Uso:

    python3 bin/python/modelconf.py dump     --root <dir_models> [--include-fired]
    python3 bin/python/modelconf.py args      --root <dir_models> --name <modelo> \
                                              [--level <nivel>] [--flag <flag>]... \
                                              [--option <k=v>]...
    python3 bin/python/modelconf.py validate --root <dir_models>

`dump` imprime un registro por modelo. El separador de campos es el byte US
(0x1F) y cada registro termina en \n. La primera línea es la constante
ROPERO_MC_DUMP_VERSION=1, para que el consumidor sepa el formato antes de leer
la primera línea de datos.

`args` imprime el argv final (UN ARGUMENTO POR LÍNEA) que se le pasaría al
motor del modelo: [args].base + [args.level.<nivel>] + [args.flag.<flag>]... +
[args.option.<opción>]...

`validate` comprueba que cada .toml cumple el esquema de models/_template.toml.
Imprime una línea por problema y sale con 1 si hay alguno.

COMPATIBILIDAD
--------------
`tomllib` sólo existe en Python >= 3.11. Este script tiene que funcionar en
Python 3.8, así que lleva su propio parser TOML mínimo (subconjunto: tablas
[a.b], tablas inline, cadenas básicas y literales, enteros, flotantes,
booleanos, arrays de cualquier tipo y arrays multilínea). Si `tomllib` está
disponible se usa (es más fiable); si no, se usa el parser propio.

Para probar el parser propio en un Python nuevo: ROPERO_FORCE_TOML_PARSER=1
"""

import argparse
import base64
import json
import os
import re
import sys

# ---------------------------------------------------------------------------
# Constantes del formato
# ---------------------------------------------------------------------------

DUMP_VERSION = 1
DUMP_HEADER = "ROPERO_MC_DUMP_VERSION=%d" % DUMP_VERSION
FIELD_SEP = "\x1f"

# Engines conocidos (spec §6). El id canónico del engine MLX es "mlx-serve";
# "mlx_lm" es la ortografía legacy y se canóniza al cargar (con aviso en
# stderr), de modo que los TOML viejos siguen valiendo sin edición.
KNOWN_ENGINES = (
    "llama-server", "vllm", "airllm", "mlx-serve", "tensorrt-llm", "fastflowlm",
)
KNOWN_ENGINE_DIRS = ("llama", "vllm", "airllm", "mlx", "tensorrt", "flm")
KNOWN_FORMATS = ("gguf", "safetensors")

# ids legacy → id canónico (spec §7.4.1).
LEGACY_ENGINES = {"mlx_lm": "mlx-serve"}

# Ficheros que [files] puede declarar. Los placeholders ${gguf}, ${mtp} y
# ${mmproj} apuntan a estos tres.
FILE_KEYS = ("gguf", "mtp", "mmproj", "chat_template")

# Claves que se aceptan en la sección [build] de un TOML de modelo.
#
# [build] describe CÓMO fabricar el GGUF, y sus claves son las MISMAS que
# acepta `ropero gguffer`. Se listan aquí para que un typo se avise al
# cargar el TOML y no 40 minutos después, cuando el cuantizador falla.
#
#   repo            de dónde bajar los PESOS (clonar y convertir)
#   gguf            fuente ya quantizada: "owner/repo" o "owner/repo:fichero"
#   gguf_file       el fichero dentro del repo, con gguf
#   quant           el quant a aplicar (o varios, separados por coma)
#   imatrix         importance matrix LOCAL (ruta)
#   imatrix_repo    de dónde bajar el imatrix
#   imatrix_file    el fichero del imatrix dentro de ese repo
#   out             nombre o ruta del GGUF de salida
#   out_principal   ruta EXACTA del primer quant de la tirada
#   from_subdir     subcarpeta del repo de la que sacar los pesos
#   include         qué ficheros descargar
#   intermedio      f16 (por defecto) o bf16
#   keep_source     conservar los safetensors
#   keep_intermedio no borrar el GGUF intermedio
#   no_mtp          excluir la cabeza MTP
#   force           rehacerlo todo desde cero
BUILD_KEYS = (
    "repo", "gguf", "gguf_file", "quant", "imatrix", "imatrix_repo",
    "imatrix_file", "out", "out_principal", "from_subdir", "include",
    "intermedio", "keep_source", "keep_intermedio", "no_mtp", "force",
    # Informativo: el tamaño de los PESOS que se bajan al fabricar. Es lo
    # que ve el usuario cuando se le pregunta "esto va a bajar 65 GB, ¿va?".
    # No es una flag del gguffer: es un dato de la receta.
    "size",
)


# ===========================================================================
# Parser TOML
# ===========================================================================
#
# Subconjunto soportado (suficiente para el esquema de models/):
#   · tablas [a.b]
#   · tablas inline {k = v, k2 = v2}
#   · cadenas básicas "..." (con escapes) y literales '...'
#   · enteros, flotantes, booleanos
#   · arrays de cualquier tipo, también multilínea
# NO soportado: arrays de tablas [[a]], fechas/horas.

class TomlError(Exception):
    """Error de sintaxis al leer un .toml con el parser propio."""


_BASIC_ESCAPES = {
    "b": "\b",
    "t": "\t",
    "n": "\n",
    "f": "\f",
    "r": "\r",
    '"': '"',
    "\\": "\\",
}

_BARE_KEY_RE = re.compile(r"[A-Za-z0-9_-]+")


def _strip_comment(text):
    """Quita el comentario final de una línea, sin tocar lo que va en comillas."""
    out = []
    i = 0
    n = len(text)
    in_basic = False
    in_literal = False
    while i < n:
        ch = text[i]
        if in_basic:
            if ch == "\\" and i + 1 < n:
                out.append(ch)
                out.append(text[i + 1])
                i += 2
                continue
            if ch == '"':
                in_basic = False
        elif in_literal:
            if ch == "'":
                in_literal = False
        else:
            if ch == '"':
                in_basic = True
            elif ch == "'":
                in_literal = True
            elif ch == "#":
                break
        out.append(ch)
        i += 1
    return "".join(out)


def _depth(text):
    """Profundidad de corchetes/llaves sin contar los que van dentro de comillas."""
    depth = 0
    i = 0
    n = len(text)
    in_basic = False
    in_literal = False
    while i < n:
        ch = text[i]
        if in_basic:
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                in_basic = False
        elif in_literal:
            if ch == "'":
                in_literal = False
        else:
            if ch == '"':
                in_basic = True
            elif ch == "'":
                in_literal = True
            elif ch in "[{":
                depth += 1
            elif ch in "]}":
                depth -= 1
        i += 1
    return depth


def _split_top_level(text, sep=","):
    """Parte un texto por `sep`, ignorando los separadores dentro de comillas."""
    parts = []
    cur = []
    depth = 0
    i = 0
    n = len(text)
    in_basic = False
    in_literal = False
    while i < n:
        ch = text[i]
        if in_basic:
            cur.append(ch)
            if ch == "\\" and i + 1 < n:
                cur.append(text[i + 1])
                i += 2
                continue
            if ch == '"':
                in_basic = False
            i += 1
            continue
        if in_literal:
            cur.append(ch)
            if ch == "'":
                in_literal = False
            i += 1
            continue
        if ch == '"':
            in_basic = True
        elif ch == "'":
            in_literal = True
        elif ch in "[{":
            depth += 1
        elif ch in "]}":
            depth -= 1
        elif ch == sep and depth == 0:
            parts.append("".join(cur))
            cur = []
            i += 1
            continue
        cur.append(ch)
        i += 1
    parts.append("".join(cur))
    return parts


def _parse_basic_string(text):
    out = []
    i = 1
    n = len(text)
    while i < n:
        ch = text[i]
        if ch == "\\":
            i += 1
            if i >= n:
                raise TomlError("cadena básica sin cerrar")
            esc = text[i]
            if esc in _BASIC_ESCAPES:
                out.append(_BASIC_ESCAPES[esc])
                i += 1
            elif esc == "u":
                out.append(chr(int(text[i + 1:i + 5], 16)))
                i += 5
            elif esc == "U":
                out.append(chr(int(text[i + 1:i + 9], 16)))
                i += 9
            else:
                raise TomlError("escape desconocido: \\%s" % esc)
            continue
        if ch == '"':
            return "".join(out)
        out.append(ch)
        i += 1
    raise TomlError("cadena básica sin cerrar: %r" % text)


def _parse_literal_string(text):
    end = text.find("'", 1)
    if end < 0:
        raise TomlError("cadena literal sin cerrar: %r" % text)
    return text[1:end]


def _parse_key_path(text):
    """Divide una clave (posiblemente con puntos) en sus partes."""
    text = text.strip()
    parts = []
    for chunk in _split_top_level(text, "."):
        chunk = chunk.strip()
        if not chunk:
            raise TomlError("clave vacía en %r" % text)
        if chunk.startswith('"'):
            value, used = _read_basic_string(chunk)
        elif chunk.startswith("'"):
            value, used = _read_literal_string(chunk)
        elif _BARE_KEY_RE.fullmatch(chunk):
            value = chunk
        else:
            raise TomlError("clave no reconocida: %r" % chunk)
        parts.append(value)
    if not parts:
        raise TomlError("clave vacía")
    return parts


def _read_basic_string(text):
    """Lee una cadena básica al principio de `text`; devuelve (valor, consumidos)."""
    out = []
    i = 1
    n = len(text)
    while i < n:
        ch = text[i]
        if ch == "\\":
            i += 1
            if i >= n:
                raise TomlError("cadena básica sin cerrar")
            esc = text[i]
            if esc in _BASIC_ESCAPES:
                out.append(_BASIC_ESCAPES[esc])
                i += 1
            elif esc == "u":
                out.append(chr(int(text[i + 1:i + 5], 16)))
                i += 5
            elif esc == "U":
                out.append(chr(int(text[i + 1:i + 9], 16)))
                i += 9
            else:
                raise TomlError("escape desconocido: \\%s" % esc)
            continue
        if ch == '"':
            return "".join(out), i + 1
        out.append(ch)
        i += 1
    raise TomlError("cadena básica sin cerrar: %r" % text)


def _read_literal_string(text):
    end = text.find("'", 1)
    if end < 0:
        raise TomlError("cadena literal sin cerrar: %r" % text)
    return text[1:end], end + 1


def _parse_number(text):
    raw = text.strip()
    low = raw.lower()
    if low in ("inf", "+inf", "-inf", "nan", "+nan", "-nan"):
        return float(low.replace("+", ""))
    clean = raw.replace("_", "")
    try:
        if re.match(r"^[+-]?0x[0-9a-fA-F]+$", clean):
            return int(clean, 16)
        if re.match(r"^[+-]?0o[0-7]+$", clean):
            return int(clean, 8)
        if re.match(r"^[+-]?0b[01]+$", clean):
            return int(clean, 2)
        if re.match(r"^[+-]?[0-9]+$", clean):
            return int(clean, 10)
        return float(clean)
    except ValueError:
        raise TomlError("valor no reconocido: %r" % text)


def _parse_value(text):
    text = text.strip()
    if not text:
        raise TomlError("valor vacío")
    if text.startswith('"'):
        value, used = _read_basic_string(text)
        if text[used:].strip():
            raise TomlError("sobra tras la cadena: %r" % text)
        return value
    if text.startswith("'"):
        value, used = _read_literal_string(text)
        if text[used:].strip():
            raise TomlError("sobra tras la cadena: %r" % text)
        return value
    if text.startswith("["):
        return _parse_array(text)
    if text.startswith("{"):
        return _parse_inline_table(text)
    low = text.lower()
    if low == "true":
        return True
    if low == "false":
        return False
    return _parse_number(text)


def _parse_array(text):
    text = text.strip()
    if not text.endswith("]"):
        raise TomlError("array sin cerrar: %r" % text)
    inner = text[1:-1]
    items = []
    for chunk in _split_top_level(inner):
        chunk = chunk.strip()
        if not chunk:
            continue
        items.append(_parse_value(chunk))
    return items


def _parse_inline_table(text):
    text = text.strip()
    if not text.endswith("}"):
        raise TomlError("tabla inline sin cerrar: %r" % text)
    inner = text[1:-1]
    table = {}
    for chunk in _split_top_level(inner):
        chunk = chunk.strip()
        if not chunk:
            continue
        eq = _find_top_level_eq(chunk)
        if eq < 0:
            raise TomlError("par clave = valor no reconocido: %r" % chunk)
        keys = _parse_key_path(chunk[:eq])
        value = _parse_value(chunk[eq + 1:])
        target = table
        for key in keys[:-1]:
            nxt = target.get(key)
            if not isinstance(nxt, dict):
                nxt = {}
                target[key] = nxt
            target = nxt
        target[keys[-1]] = value
    return table


def _find_top_level_eq(text):
    i = 0
    n = len(text)
    in_basic = False
    in_literal = False
    while i < n:
        ch = text[i]
        if in_basic:
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                in_basic = False
        elif in_literal:
            if ch == "'":
                in_literal = False
        else:
            if ch == '"':
                in_basic = True
            elif ch == "'":
                in_literal = True
            elif ch == "=":
                return i
        i += 1
    return -1


def _descend(root, keys):
    target = root
    for key in keys:
        nxt = target.get(key)
        if not isinstance(nxt, dict):
            nxt = {}
            target[key] = nxt
        target = nxt
    return target


def parse_toml(text):
    """Parser TOML mínimo. Devuelve un dict."""
    root = {}
    current = root
    lines = text.splitlines()
    index = 0
    while index < len(lines):
        line = _strip_comment(lines[index]).strip()
        index += 1
        if not line:
            continue
        if line.startswith("[["):
            raise TomlError("las tablas de array [[...]] no están soportadas")
        if line.startswith("["):
            end = line.find("]")
            if end < 0:
                raise TomlError("cabecera de tabla sin cerrar: %r" % line)
            current = _descend(root, _parse_key_path(line[1:end]))
            continue
        eq = _find_top_level_eq(line)
        if eq < 0:
            raise TomlError("línea no reconocida: %r" % line)
        keys = _parse_key_path(line[:eq])
        value_text = line[eq + 1:].strip()
        # Un valor puede ocupar varias líneas (arrays multilínea).
        while _depth(value_text) > 0:
            if index >= len(lines):
                raise TomlError("valor sin cerrar: %r" % value_text)
            value_text += " " + _strip_comment(lines[index]).strip()
            index += 1
        target = current
        for key in keys[:-1]:
            nxt = target.get(key)
            if not isinstance(nxt, dict):
                nxt = {}
                target[key] = nxt
            target = nxt
        target[keys[-1]] = _parse_value(value_text)
    return root


def _tomllib():
    """Devuelve el módulo tomllib, o None si no está (o si se fuerza el parser)."""
    if os.environ.get("ROPERO_FORCE_TOML_PARSER", "") == "1":
        return None
    try:
        import tomllib
    except ImportError:
        return None
    return tomllib


def load_toml(path):
    """Carga un .toml. Usa tomllib si está; si no, el parser propio."""
    with open(path, "rb") as handle:
        raw = handle.read()
    module = _tomllib()
    if module is not None:
        return module.loads(raw.decode("utf-8"))
    return parse_toml(raw.decode("utf-8"))


# ===========================================================================
# Utilidades
# ===========================================================================

def _clean(value):
    """Deja un valor listo para el dump: una línea, sin separadores de campo."""
    if value is None:
        return ""
    text = value if isinstance(value, str) else str(value)
    text = text.replace("\r", " ").replace("\n", " ").replace(FIELD_SEP, " ")
    return text.strip()


def _as_int(value):
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    try:
        return int(str(value).strip())
    except (TypeError, ValueError):
        return None


def _as_list(value):
    if value is None:
        return []
    if isinstance(value, (list, tuple)):
        return [v for v in value]
    return [value]


def _b64_json(obj):
    if obj is None:
        return ""
    payload = json.dumps(obj, ensure_ascii=False, separators=(",", ":"), sort_keys=False)
    return base64.b64encode(payload.encode("utf-8")).decode("ascii")


def _gguf_dir():
    """$GGUF_DIR, o $HOME/models/gguf si no está definida."""
    directory = os.environ.get("GGUF_DIR", "")
    if not directory:
        home = os.environ.get("HOME", "")
        directory = os.path.join(home, "models", "gguf") if home else "models/gguf"
    return os.path.abspath(os.path.expanduser(directory))


def _safetensors_dir():
    """$SAFETENSORS_DIR, o $HOME/models/safetensors si no está definida."""
    directory = os.environ.get("SAFETENSORS_DIR", "")
    if not directory:
        home = os.environ.get("HOME", "")
        directory = os.path.join(home, "models", "safetensors") if home else "models/safetensors"
    return os.path.abspath(os.path.expanduser(directory))


def ctx_env_name(model_name):
    """Nombre de la variable de entorno que pisa el contexto de un modelo."""
    slug = re.sub(r"[^A-Za-z0-9]+", "_", model_name or "").strip("_").upper()
    return "ROPERO_%s_CTX" % slug


def gguf_env_name(model_name):
    """Nombre de la variable de entorno que pisa el fichero GGUF de un modelo.

    Permite usar otra cuantización del mismo repo sin tocar el TOML:
    ROPERO_GPTOSS_GGUF=openai_gpt-oss-20b-MXFP4.gguf.
    """
    slug = re.sub(r"[^A-Za-z0-9]+", "_", model_name or "").strip("_").upper()
    return "ROPERO_%s_GGUF" % slug


def effective_context(model):
    """[model].context, salvo que ROPERO_<NOMBRE>_CTX lo pise."""
    context = _as_int(model.get("context"))
    if context is None:
        return None
    override = os.environ.get(ctx_env_name(model.get("name", "")))
    if override:
        parsed = _as_int(override)
        if parsed is not None:
            return parsed
    return context


# ===========================================================================
# Descubrimiento de modelos
# ===========================================================================

def iter_model_files(root, include_fired=False):
    """Devuelve [(ruta_abs, engine_dir, origin)] de los .toml de modelo."""
    root = os.path.abspath(root)
    found = []
    try:
        entries = sorted(os.listdir(root))
    except OSError as exc:
        raise ModelConfError("no se pudo leer el directorio de modelos %s: %s" % (root, exc))
    for entry in entries:
        engine_dir = os.path.join(root, entry)
        if not os.path.isdir(engine_dir):
            continue
        for name in sorted(os.listdir(engine_dir)):
            path = os.path.join(engine_dir, name)
            if name.startswith("_") or not name.endswith(".toml"):
                continue
            if not os.path.isfile(path):
                continue
            found.append((os.path.abspath(path), entry, "active"))
        fired_dir = os.path.join(engine_dir, "fired")
        if include_fired and os.path.isdir(fired_dir):
            for name in sorted(os.listdir(fired_dir)):
                path = os.path.join(fired_dir, name)
                if name.startswith("_") or not name.endswith(".toml"):
                    continue
                if not os.path.isfile(path):
                    continue
                found.append((os.path.abspath(path), entry, "fired"))
    return found


class ModelConfError(Exception):
    """Error de uso o de datos; se imprime por stderr y se sale con 1."""


def load_model(path):
    """Carga un .toml de modelo y normaliza sus secciones."""
    try:
        data = load_toml(path)
    except TomlError as exc:
        raise ModelConfError("%s: TOML inválido: %s" % (path, exc))
    except ValueError as exc:
        raise ModelConfError("%s: TOML inválido: %s" % (path, exc))
    if not isinstance(data, dict):
        raise ModelConfError("%s: el TOML no es una tabla" % path)
    model = data.get("model") or {}
    # id legacy → canónico (spec §7.4.1). "mlx_lm" se ACEPTA y se traduce a
    # "mlx-serve" con un aviso en stderr: el que lo ve sabe que su TOML usa
    # la ortografía vieja. El resto del código sólo ve ids canónicos.
    raw_engine = model.get("engine")
    if isinstance(raw_engine, str) and raw_engine in LEGACY_ENGINES:
        canonical = LEGACY_ENGINES[raw_engine]
        model["engine"] = canonical
        sys.stderr.write(
            "AVISO: %s: engine '%s' es legacy; se canóniza a '%s'\n"
            % (path, raw_engine, canonical))
    files = data.get("files") or {}
    download = data.get("download") or {}
    levels = data.get("levels") or {}
    flags = data.get("flags") or {}
    options = data.get("options") or {}
    aliases = data.get("aliases") or {}
    args = data.get("args") or {}
    limits = data.get("limits") or {}
    aux = download.get("aux") or {}
    build = data.get("build") or {}
    return {
        "path": os.path.abspath(path),
        "data": data,
        "model": model,
        "files": files,
        "download": download,
        "levels": levels,
        "flags": flags,
        "options": options,
        "options_default": options.get("default") or {},
        "aliases": aliases,
        "args": args,
        "limits": limits,
        "aux": aux,
        "build": build,
    }


def find_model(root, name):
    """Busca un modelo por nombre. Primero los activos; los retired después."""
    entries = iter_model_files(root, include_fired=True)
    actives = [e for e in entries if e[2] == "active"]
    fired = [e for e in entries if e[2] == "fired"]
    for group in (actives, fired):
        matches = []
        for path, engine_dir, origin in group:
            try:
                loaded = load_model(path)
            except ModelConfError:
                continue
            if loaded["model"].get("name") == name:
                matches.append((path, engine_dir, origin))
        if len(matches) == 1:
            return matches[0]
        if len(matches) > 1:
            raise ModelConfError(
                "el nombre '%s' está duplicado: %s" % (
                    name, ", ".join(m[0] for m in matches)))
    raise ModelConfError("modelo desconocido: '%s'" % name)


# ===========================================================================
# Montaje de los argumentos
# ===========================================================================

def _resolve_token(token, ctx):
    """Resuelve un placeholder: ${gguf}, ${ctx}, ${value}, ${env:VAR}, ..."""
    if token.startswith("env:"):
        spec = token[4:]
        if ":-" in spec:
            var, default = spec.split(":-", 1)
            value = ctx["environ"].get(var)
            if value is None or value == "":
                return _expand_home(default)
            return value
        value = ctx["environ"].get(spec)
        if value is None:
            return "${env:%s}" % spec
        return value
    if token == "gguf":
        return ctx["gguf"]
    if token == "mtp":
        return ctx["mtp"]
    if token == "mmproj":
        return ctx["mmproj"]
    if token == "chat_template":
        return ctx["chat_template"]
    if token == "ctx":
        return "" if ctx["ctx"] is None else str(ctx["ctx"])
    if token == "value":
        return ctx.get("value", "")
    if token == "model_path":
        # $SAFETENSORS_DIR/<[model].path> (spec §7.2). Sólo tiene valor en un
        # modelo safetensors que declare [model].path.
        return ctx.get("model_path", "")
    return "${%s}" % token


def _expand_home(text):
    """Expande $HOME y ${HOME} en un valor (el defecto de ${env:VAR:-defecto}).

    El defecto puede llevar $HOME (p.ej. una ruta de shards bajo el home del
    usuario), y tiene que salir expandido a la HOME real del proceso.
    """
    if not isinstance(text, str) or "$" not in text:
        return text
    home = os.environ.get("HOME", "")
    text = text.replace("${HOME}", home).replace("$HOME", home)
    return text


def expand(text, ctx):
    """Sustituye los placeholders de una cadena."""
    if not isinstance(text, str) or "${" not in text:
        return text
    out = []
    i = 0
    while True:
        start = text.find("${", i)
        if start < 0:
            out.append(text[i:])
            break
        out.append(text[i:start])
        end = text.find("}", start)
        if end < 0:
            out.append(text[start:])
            break
        out.append(_resolve_token(text[start + 2:end], ctx))
        i = end + 1
    return "".join(out)


def _slug_upper(text):
    """'n-cpu-moe' → 'N_CPU_MOE' (el nombre de la variable de entorno)."""
    return re.sub(r"[^A-Za-z0-9]+", "_", text or "").strip("_").upper()


def _drop_flag_pair(argv, flag):
    """Quita de `argv` la pareja <flag> <valor>.

    Se usa para que una opción con `replace` sustituya al flag del base en
    lugar de duplicarlo. Si el flag no está, no hace nada (no es un error: el
    modelo puede no llevar ese flag en su base).
    """
    try:
        index = argv.index(flag)
    except ValueError:
        return
    del argv[index:index + 2]


def _expand_all(items, ctx):
    return [expand(item, ctx) for item in items]


def _chat_template_pair(cfg, ctx=None):
    """Devuelve (flag, json) del atajo chat_template_kwargs / reasoning_effort.

    Con ctx, los valores se expanden: así el TOML puede fijar el nivel por
    defecto ("low"/"medium"/"high") y dejar que el entorno lo sobreescriba con
    ${env:VAR:-<defecto>}.

    Antes NO se expandían, y eso mataba al env var: el TOML ponía el flag a
    mano en [args].base y el nivel lo volvía a poner. llama.cpp avisa
    "DEPRECATED: argument '--chat-template-kwargs' specified multiple times" y
    se queda con el ÚLTIMO, que es el del nivel: el valor del entorno se
    descartaba en silencio.
    """
    if "chat_template_kwargs" in cfg and cfg["chat_template_kwargs"] is not None:
        payload = cfg["chat_template_kwargs"]
        if ctx is not None:
            payload = {
                k: (expand(v, ctx) if isinstance(v, str) else v)
                for k, v in payload.items()
            }
        return "--chat-template-kwargs", json.dumps(
            payload, ensure_ascii=False, separators=(",", ":"))
    effort = cfg.get("reasoning_effort")
    if effort:
        if ctx is not None:
            effort = expand(str(effort), ctx)
        payload = {"reasoning_effort": effort}
        return "--chat-template-kwargs", json.dumps(
            payload, ensure_ascii=False, separators=(",", ":"))
    return None, None


def build_argv(loaded, level=None, flags=(), options=()):
    """Monta el argv final del modelo: base + nivel + flags + opciones."""
    model = loaded["model"]
    files = loaded["files"]
    args_spec = loaded["args"]
    levels_spec = args_spec.get("level") or {}
    flags_spec = args_spec.get("flag") or {}
    options_spec = args_spec.get("option") or {}
    levels_list = _as_list(loaded["levels"].get("list"))

    # ── Nivel ──────────────────────────────────────────────────────────────
    if level is not None:
        if not levels_list:
            raise ModelConfError(
                "el modelo '%s' no declara niveles; no se puede pedir '%s'"
                % (model.get("name", "?"), level))
        if level not in levels_list:
            raise ModelConfError(
                "nivel de razonamiento inválido: '%s' (válidos: %s)"
                % (level, " ".join(levels_list)))
    elif levels_list:
        default = loaded["levels"].get("default")
        if default and default in levels_list:
            level = default

    ctx = {
        "environ": os.environ,
        "gguf": "",
        "mtp": "",
        "mmproj": "",
        "chat_template": "",
        "ctx": effective_context(model),
        "value": "",
        "model_path": "",
    }
    gguf_dir = _gguf_dir()
    model_dir = _clean(model.get("name", "")) or "model"
    if model.get("path") and (model.get("format") or "gguf") == "safetensors":
        # ${model_path}: el checkpoint entero, relativo a $SAFETENSORS_DIR
        # (spec §7.2). Es el equivalente safetensors de ${gguf}.
        ctx["model_path"] = os.path.join(_safetensors_dir(), str(model.get("path")))
    if files.get("gguf"):
        # ROPERO_<NOMBRE>_GGUF permite apuntar el modelo a otra cuantización
        # del MISMO repo sin tocar el TOML. Antes vivía en el MODEL_GGUF del
        # script ("MODEL_GGUF=${ROPERO_GPTOSS_GGUF:-gpt-oss-...}"); ahora el
        # override es genérico y lo declara el nombre del modelo.
        gguf_name = os.environ.get(gguf_env_name(model.get("name", ""))) or files["gguf"]
        ctx["gguf"] = os.path.join(gguf_dir, model_dir, gguf_name)
    if files.get("mtp"):
        ctx["mtp"] = os.path.join(gguf_dir, model_dir, files["mtp"])
    if files.get("mmproj"):
        ctx["mmproj"] = os.path.join(gguf_dir, model_dir, files["mmproj"])
    if files.get("chat_template"):
        ctx["chat_template"] = os.path.join(gguf_dir, model_dir, files["chat_template"])

    argv = []

    # 1. [args].base
    base = args_spec.get("base")
    if base is not None:
        if not isinstance(base, list):
            raise ModelConfError("[args].base debe ser una lista")
        argv.extend(_expand_all(base, ctx))

    # 2. [args.level.<nivel>]
    if level is not None and level in levels_spec:
        cfg = levels_spec[level] or {}
        level_base = cfg.get("base") or []
        argv.extend(_expand_all(level_base, ctx))
        flag, payload = _chat_template_pair(cfg, ctx)
        if flag:
            argv.append(flag)
            argv.append(payload)

    # 3. [args.flag.<flag>] por cada flag pedido, en el orden pedido
    for name in flags:
        if name not in flags_spec:
            raise ModelConfError(
                "el modelo '%s' no acepta el flag '%s' (válidos: %s)"
                % (model.get("name", "?"), name,
                   " ".join(_as_list(loaded["flags"].get("list"))) or "-"))
        cfg = flags_spec[name] or {}
        argv.extend(_expand_all(cfg.get("base") or [], ctx))

    # 4. [args.option.<opción>] por cada opción pedida
    #
    # Las opciones se resuelven en este orden de prioridad:
    #   1. lo que el usuario ha escrito en la línea de órdenes (k=v)
    #   2. la variable de entorno ROPERO_MODEL_OPT_<K> (la pone el CLI)
    #   3. [options.default] del TOML
    #
    # Una opción con `replace = "--flag"` SUSTITUYE la aparición de ese flag en
    # el base en lugar de añadirse detrás. Duplicar el flag funciona (llama.cpp
    # se queda con el último), pero es una trampa: el argv acaba con dos
    # "--n-cpu-moe" y cualquier lectura ingenua del argv ve el equivocado.
    #
    # El valor por DEFECTO no sustituye nada: el base ya lleva el valor por
    # defecto, y suele llevar además su propio override por entorno
    # (${env:ROPERO_GEMMA_N_CPU_MOE:-5}). Si el default sustituyera, ese
    # override se perdería.
    resolved = {}
    for request in options:
        key, sep, value = request.partition("=")
        key = key.strip()
        if not sep:
            raise ModelConfError("opción mal formada: '%s' (se espera k=v)" % request)
        if key not in options_spec:
            raise ModelConfError(
                "el modelo '%s' no acepta la opción '%s' (válidas: %s)"
                % (model.get("name", "?"), key,
                   " ".join(_as_list(loaded["options"].get("list"))) or "-"))
        resolved[key] = (value, True)

    # Las opciones que el CLI ha exportado al entorno, y que el usuario no ha
    # escrito en la línea de órdenes, cuentan igual.
    for key in options_spec:
        if key in resolved:
            continue
        env_value = os.environ.get("ROPERO_MODEL_OPT_%s" % _slug_upper(key))
        if env_value:
            resolved[key] = (env_value, True)

    for key in options_spec:
        if key not in resolved:
            continue
        value, explicit = resolved[key]
        ctx["value"] = value
        cfg = options_spec[key] or {}
        replace = _clean(cfg.get("replace", ""))
        opt_args = _expand_all(cfg.get("base") or [], ctx)
        if replace and explicit:
            _drop_flag_pair(argv, replace)
        argv.extend(opt_args)

    return argv


# ===========================================================================
# Dump
# ===========================================================================

def build_record(path, engine_dir, origin):
    """Construye el registro de un modelo (los 27 campos del dump)."""
    loaded = load_model(path)
    model = loaded["model"]
    files = loaded["files"]
    download = loaded["download"]
    aux = loaded["aux"]
    args_spec = loaded["args"]

    name = _clean(model.get("name", ""))
    engine = _clean(model.get("engine", ""))
    alias = _clean(model.get("alias", ""))
    context = effective_context(model)
    port = _as_int(model.get("port"))
    fmt = _clean(model.get("format") or "gguf")
    model_path = _clean(model.get("path", ""))

    gguf = _clean(files.get("gguf", ""))
    mtp = _clean(files.get("mtp", ""))
    mmproj = _clean(files.get("mmproj", ""))

    repo = _clean(download.get("repo", ""))
    file_name = _clean(download.get("file", ""))
    dest_name = _clean(download.get("name", "")) or file_name
    size = _clean(download.get("size", ""))
    excludes = ",".join(_clean(e) for e in _as_list(download.get("excludes")))

    levels = " ".join(_clean(x) for x in _as_list(loaded["levels"].get("list")))
    level_default = _clean(loaded["levels"].get("default", ""))
    flags = " ".join(_clean(x) for x in _as_list(loaded["flags"].get("list")))
    options = " ".join(_clean(x) for x in _as_list(loaded["options"].get("list")))
    aliases = ",".join("%s=%s" % (_clean(k), _clean(v))
                       for k, v in loaded["aliases"].items())

    def aux_field(kind):
        cfg = aux.get(kind) or {}
        if not cfg:
            return ""
        aux_file = _clean(cfg.get("file", ""))
        aux_name = _clean(cfg.get("name", "")) or aux_file
        aux_size = _clean(cfg.get("size", ""))
        # El aux puede vivir en OTRO repo que el del modelo: el draft MTP
        # del nemotron esta en tngtech y el MXFP4 en vcruz305. El que
        # declara el TOML manda; si no lo declara, es el repo del modelo.
        aux_repo = _clean(cfg.get("repo", "")) or repo
        # Cuando se usa: por defecto, el del tipo. El TOML puede cambiarlo.
        aux_use = _clean(cfg.get("use", "")) or AUX_USE_DEFAULT.get(kind, "run")
        if aux_use not in AUX_USES:
            aux_use = AUX_USE_DEFAULT.get(kind, "run")
        return "|".join([aux_repo, aux_file, aux_name, aux_size, aux_use])

    # La spec de [args], serializada a JSON y en base64.
    spec = {}
    if "base" in args_spec:
        spec["base"] = args_spec["base"]
    levels_spec = args_spec.get("level") or {}
    if levels_spec:
        spec["levels"] = {}
        for name_level, cfg in levels_spec.items():
            cfg = cfg or {}
            entry = {"base": cfg.get("base") or []}
            if "chat_template_kwargs" in cfg:
                entry["chat_template_kwargs"] = cfg["chat_template_kwargs"]
            if "reasoning_effort" in cfg:
                entry["reasoning_effort"] = cfg["reasoning_effort"]
            spec["levels"][name_level] = entry
    flags_spec = args_spec.get("flag") or {}
    if flags_spec:
        spec["flags"] = {}
        for name_flag, cfg in flags_spec.items():
            cfg = cfg or {}
            entry = {"base": cfg.get("base") or []}
            if "requires" in cfg:
                entry["requires"] = cfg["requires"]
            spec["flags"][name_flag] = entry
    options_spec = args_spec.get("option") or {}
    if options_spec:
        spec["options"] = {}
        for name_option, cfg in options_spec.items():
            cfg = cfg or {}
            entry = {"base": cfg.get("base") or []}
            if "replace" in cfg:
                entry["replace"] = _clean(cfg.get("replace"))
            spec["options"][name_option] = entry

    opts_default = loaded["options_default"]
    opts_default_b64 = ""
    if opts_default:
        opts_default_b64 = _b64_json(
            dict((_clean(k), v) for k, v in opts_default.items()))

    limits = loaded["limits"]
    limits_b64 = ""
    if limits:
        limits_b64 = _b64_json(dict((_clean(k), v) for k, v in limits.items()))

    # ── La spec de [build] ──────────────────────────────────────────────────
    #
    # [build] dice CÓMO se fabrica el GGUF de este modelo, en vez de DÓNDE
    # se descarga. Los campos son los mismos que acepta `ropero gguffer`, y
    # el que se ejecuta es el gguffer de verdad: una sola implementación.
    #
    # Va serializada en JSON+base64 como la spec de [args] y los [limits],
    # porque son datos estructurados (listas, booleanos) y no caben bien en
    # un campo de texto separado por \x1f.
    build = loaded["build"]
    build_b64 = ""
    if build:
        build_spec = {}
        for key, value in build.items():
            key = _clean(key)
            if key in BUILD_KEYS:
                build_spec[key] = value
        if build_spec:
            build_b64 = _b64_json(build_spec)

    fields = [
        name,
        engine,
        alias,
        origin,
        engine_dir,
        "" if context is None else str(context),
        "" if port is None else str(port),
        fmt,
        model_path,
        gguf,
        mtp,
        mmproj,
        repo,
        file_name,
        dest_name,
        size,
        excludes,
        levels,
        level_default,
        flags,
        options,
        aliases,
        aux_field("mtp"),
        aux_field("mmproj"),
        _b64_json(spec) if spec else "",
        opts_default_b64,
        limits_b64,
        # `manual`: el modelo existe y se puede arrancar, pero su descarga NO
        # se ofrece en `ropero install models`. Es el caso de los checkpoints
        # enormes (airllm: 130 GB), que no son una descarga que un install
        # pueda hacer por sorpresa. El dato va en el TOML, no en el shell.
        "1" if download.get("manual") else "0",
        os.path.abspath(path),
        # `chat_template`: fichero auxiliar (plantilla Jinja) que se descarga
        # con el bundle y se referencia desde [args] con ${chat_template}.
        aux_field("chat_template"),
        # `build`: cómo se fabrica el GGUF (los parámetros del gguffer). Si
        # está vacío, el modelo se DESCARGA de [download]; si no, se
        # FABRICA con el gguffer.
        build_b64,
    ]
    return fields


def cmd_dump(opts):
    entries = iter_model_files(opts.root, include_fired=opts.include_fired)
    out = sys.stdout
    out.write(DUMP_HEADER + "\n")
    # Un TOML roto NO puede llevarse por delante el catálogo entero.
    #
    # Antes, una excepción al construir un registro abortaba el dump: el CLI se
    # quedaba sin NINGÚN modelo y `ropero list` imprimía una lista vacía
    # (con código de salida 0, para más gracia). Un error de sintaxis en un
    # solo fichero era indistinguible de "no hay modelos instalados".
    #
    # Ahora cada fichero se aísla: el que está mal se avisa en stderr y se
    # salta, y los demás se cargan igual. El aviso va a stderr para no
    # contaminar el dump, que se lee de stdout.
    failed = 0
    for path, engine_dir, origin in entries:
        try:
            fields = build_record(path, engine_dir, origin)
        except (ModelConfError, Exception) as exc:  # noqa: BLE001
            failed += 1
            sys.stderr.write("ERROR: %s: %s\n" % (path, exc))
            continue
        # Las claves desconocidas de [build] se avisan aqui tambien: el dump
        # es lo que lee `ropero list`, y es donde se veria un "imatrix_repos"
        # mal escrito (que fabricaria el GGUF sin importance matrix).
        try:
            for key in unknown_build_keys(load_model(path)["build"]):
                sys.stderr.write(
                    "AVISO: %s: [build] tiene una clave desconocida: %s\n"
                    % (path, key))
        except (ModelConfError, Exception):  # noqa: BLE001
            pass
        out.write(FIELD_SEP.join(fields) + "\n")
    out.flush()
    if failed:
        sys.stderr.write("%d modelo(s) no se pudieron leer\n" % failed)
    return 0


# ===========================================================================
# Args
# ===========================================================================

def cmd_args(opts):
    path, _engine_dir, _origin = find_model(opts.root, opts.name)
    loaded = load_model(path)
    argv = build_argv(loaded, level=opts.level, flags=opts.flag or [],
                      options=opts.option or [])
    out = sys.stdout
    for item in argv:
        out.write("%s\n" % item)
    out.flush()
    return 0


# ===========================================================================
# Build
# ===========================================================================
#
# El subcomando "build" imprime CÓMO se fabrica el GGUF de un modelo: los
# parámetros de su sección [build], traducidos al argv del gguffer.
#
# Por qué un subcomando y no un campo más del dump:
#   · el dump es un registro por línea, y esta spec es una estructura;
#   · quien lo consume es el gguffer, no el catálogo de descargas;
#   · y así el argv se construye en UN sitio (python), no en bash y en
#     python a la vez, que es como se desincronizan estas cosas.
#
# El argv sale UNA OPCIÓN POR LÍNEA, como el de "args": el shell lo lee con
# mapfile y lo ejecuta sin word-splitting, de modo que un valor con
# espacios (una ruta con espacios) no se pierde.
#
# --out se calcula aquí, no en el TOML: el destino del GGUF fabricationado
# lo decide la instalación (dónde va el fichero del modelo), y meterlo en
# el TOML duplicaría la verdad.

# Las opciones de [build] que son banderas (no llevan valor).
_BUILD_FLAGS = ("keep_source", "keep_intermedio", "no_mtp", "force")

# Las opciones de [build] que llevan valor, y cómo se pasan al gguffer.
_BUILD_OPTS = (
    ("repo", "--repo"),
    ("gguf", "--gguf"),
    ("gguf_file", "--gguf-file"),
    ("imatrix", "--imatrix"),
    ("imatrix_repo", "--imatrix-repo"),
    ("imatrix_file", "--imatrix-file"),
    ("out", "--out"),
    ("out_principal", "--out-principal"),
    ("from_subdir", "--from-subdir"),
    ("include", "--include"),
    ("intermedio", "--intermedio"),
)


# ===========================================================================
# Los EXTRAS: ficheros que van con el modelo, en su misma carpeta
# ===========================================================================
#
# Un modelo no es solo el GGUF: al lado van el modelo de draft de la MTP, el
# proyector de vision, el chat template, la importance matrix... Todos ellos
# se bajan a la MISMA carpeta que el GGUF (la que declara el [files].gguf), y
# se bajan tanto si el GGUF se DESCARGA como si se FABRICA.
#
# Cada tipo de extra se usa en un momento distinto, y de eso depende de que
# el flujo sea el de siempre:
#
#   tipo             se usa en            por que
#   ---------------  ------------------  ------------------------------------
#   mtp              el arranque          --model-draft
#   mmproj           el arranque          --mmproj
#   chat_template    al fabricar           el conversor lo lee y lo mete en el
#                                          GGUF (tokenizer.chat_template)
#   imatrix          al fabricar           llama-quantize lo usa
#   tokenizer        al fabricar           el conversor lo lee del clon
#
# El "use" se puede declarar en el TOML; si no, es el de por defecto del
# tipo.
AUX_KINDS = ("mtp", "mmproj", "chat_template", "imatrix", "tokenizer")

AUX_USE_DEFAULT = {
    "mtp": "run",
    "mmproj": "run",
    "chat_template": "build",
    "imatrix": "build",
    "tokenizer": "build",
}

AUX_USES = ("build", "run")


def build_argv_for(spec, out_path=None):
    """Traduce la spec de [build] al argv del gguffer.

    Devuelve una lista de strings. Los valores booleanos True/False se
    traducen a la bandera o se omiten; los strings van tal cual.
    """
    argv = []
    for key, flag in _BUILD_OPTS:
        if key not in spec:
            continue
        value = spec[key]
        if value is None:
            continue
        if isinstance(value, bool):
            # Una opción con valor que llegó como booleano es un error de
            # escritura: se ignora en vez de pasarle "True" al gguffer.
            continue
        value = str(value).strip()
        if not value:
            continue
        argv.extend([flag, value])
    for key in _BUILD_FLAGS:
        if spec.get(key):
            argv.append("--" + key.replace("_", "-"))
    # El quant: puede ser uno o varios (lista, o string con comas).
    quant = spec.get("quant")
    if quant:
        if isinstance(quant, list):
            values = [str(q).strip() for q in quant if str(q).strip()]
        else:
            values = [q.strip() for q in str(quant).split(",") if q.strip()]
        for value in values:
            argv.extend(["--quant", value])
    # El destino. Lo pone la instalación, no el TOML.
    if out_path:
        argv.extend(["--out", out_path])
    return argv


def unknown_build_keys(build):
    """Las claves de [build] que el gguffer no entiende.

    Se comprueba en TODOS los caminos (el dump, el subcomando build y el
    validate) porque una clave mal escrita es un fallo silencioso: si
    "imatrix_repos" se cuela por "imatrix_repo", el GGUF se fabrica SIN
    importance matrix, y el error sale 40 minutos después, cuando el
    cuantizador dice que falta la matriz.
    """
    if not build:
        return []
    return [k for k in build if k not in BUILD_KEYS]


def cmd_aux(opts):
    """Imprime los EXTRAS de un modelo: tipo, repo, fichero, nombre y uso.

    Una linea por extra, con los campos separados por "|". Es lo que leen
    el instalador (para bajarlos) y el launcher (para saber cuales van en
    el arranque y cuales en la fabricacion).
    """
    path, _engine_dir, _origin = find_model(opts.root, opts.name)
    loaded = load_model(path)
    aux = loaded["aux"]
    for kind in AUX_KINDS:
        cfg = aux.get(kind) or {}
        if not cfg:
            continue
        aux_file = _clean(cfg.get("file", ""))
        if not aux_file:
            continue
        aux_name = _clean(cfg.get("name", "")) or aux_file
        aux_size = _clean(cfg.get("size", ""))
        aux_repo = _clean(cfg.get("repo", "")) or _clean(loaded["download"].get("repo", ""))
        aux_use = _clean(cfg.get("use", "")) or AUX_USE_DEFAULT.get(kind, "run")
        if aux_use not in AUX_USES:
            aux_use = AUX_USE_DEFAULT.get(kind, "run")
        sys.stdout.write("|".join([kind, aux_repo, aux_file, aux_name, aux_size, aux_use]) + "\n")
    sys.stdout.flush()
    return 0


def cmd_build(opts):
    path, _engine_dir, _origin = find_model(opts.root, opts.name)
    loaded = load_model(path)
    build = loaded["build"]
    for key in unknown_build_keys(build):
        sys.stderr.write(
            "AVISO: %s: [build] tiene una clave desconocida: %s\n" % (path, key))
    spec = {}
    for key, value in build.items():
        if key in BUILD_KEYS:
            spec[key] = value
    if not spec:
        # Un modelo sin [build] se DESCARGA. Decirlo es mejor que imprimir
        # un argv vacío, que el que lo llama tomaría por "no hay nada que
        # fabricar" sin saber que el error fue otro.
        sys.stderr.write(
            "ERROR: %s: este modelo no se fabrica: no tiene [build].\n"
            "       Se descarga de su [download].\n" % path)
        return 1
    if opts.format == "json":
        sys.stdout.write(json.dumps(spec, sort_keys=True) + "\n")
        sys.stdout.flush()
        return 0
    for item in build_argv_for(spec, out_path=opts.out):
        sys.stdout.write("%s\n" % item)
    sys.stdout.flush()
    return 0


# ===========================================================================
# Validate
# ===========================================================================

def validate_model(path, engine_dir, origin):
    """Devuelve (problemas, avisos) de un .toml de modelo.

    Los problemas hacen fallar el validate (salida 1). Los avisos se imprime
    pero no rompen: un modelo con servidor propio sin flags puede ser
    válido (p.ej. un modelo retirado), sólo suele ser un olvido.
    """
    problems = []
    warnings = []
    rel = os.path.basename(path)

    def bad(message):
        problems.append("%s: %s" % (rel, message))

    def warn(message):
        warnings.append("%s: %s" % (rel, message))

    try:
        loaded = load_model(path)
    except ModelConfError as exc:
        return [str(exc)], []
    except TomlError as exc:
        return ["%s: TOML inválido: %s" % (rel, exc)], []

    model = loaded["model"]
    files = loaded["files"]
    download = loaded["download"]
    build = loaded["build"]
    levels = loaded["levels"]
    flags = loaded["flags"]
    options = loaded["options"]
    aliases = loaded["aliases"]
    args_spec = loaded["args"]

    # ── [model] ────────────────────────────────────────────────────────────
    if "model" not in loaded["data"]:
        bad("falta la tabla [model]")
    else:
        name = model.get("name")
        if not name:
            bad("falta [model].name")
        engine = model.get("engine")
        if not engine:
            bad("falta [model].engine")
        elif engine not in KNOWN_ENGINES:
            bad("[model].engine desconocido: '%s' (válidos: %s)"
                % (engine, ", ".join(KNOWN_ENGINES)))
        if model.get("context") is None:
            bad("falta [model].context")
        elif _as_int(model.get("context")) is None:
            bad("[model].context no es un entero: %r" % (model.get("context"),))
        port = model.get("port")
        if port is not None and _as_int(port) is None:
            bad("[model].port no es un entero: %r" % (port,))
        fmt = model.get("format") or "gguf"
        if fmt not in KNOWN_FORMATS:
            bad("[model].format desconocido: '%s' (válidos: %s)"
                % (fmt, ", ".join(KNOWN_FORMATS)))
        if fmt == "safetensors" and not model.get("path"):
            bad("falta [model].path (obligatorio con format = safetensors)")
        if fmt == "gguf" and model.get("path"):
            bad("[model].path no tiene sentido con format = gguf")

    # ── [files] ────────────────────────────────────────────────────────────
    fmt = (model.get("format") or "gguf") if model else "gguf"
    if "files" not in loaded["data"]:
        bad("falta la tabla [files]")
    else:
        for key in FILE_KEYS:
            value = files.get(key)
            if value is None:
                continue
            if not isinstance(value, str) or not value.strip():
                bad("[files].%s está vacío o no es una cadena" % key)
            elif os.path.isabs(value):
                bad("[files].%s debe ser un nombre de fichero, no una ruta absoluta: %s"
                    % (key, value))
        if fmt == "gguf" and not files.get("gguf"):
            bad("falta [files].gguf (obligatorio con format = gguf)")

    # ── [download] ─────────────────────────────────────────────────────────
    if download:
        if not download.get("repo"):
            bad("falta [download].repo")
        # [download].file es obligatoria SÓLO con format = gguf (spec §7.4.3):
        # con safetensors el checkpoint entero se baja con `hf download` y el
        # nombre del fichero no significa nada; si está, suele ser un resto de
        # la conversión de un TOML de gguf, y se avisa (no rompe).
        if not download.get("file"):
            if fmt == "gguf":
                bad("falta [download].file (obligatoria con format = gguf)")
        elif fmt == "safetensors":
            warn("[download].file se ignora con format = safetensors "
                 "(el checkpoint entero se baja con `hf download`)")
        if download.get("name") and os.path.isabs(str(download.get("name"))):
            bad("[download].name debe ser un nombre de fichero, no una ruta absoluta")
        aux = download.get("aux") or {}
        for kind in ("mtp", "mmproj", "chat_template"):
            cfg = aux.get(kind)
            if not cfg:
                continue
            if not cfg.get("file"):
                bad("falta [download.aux.%s].file" % kind)
            if cfg.get("name") and os.path.isabs(str(cfg.get("name"))):
                bad("[download.aux.%s].name debe ser un nombre de fichero" % kind)
    elif fmt == "gguf" and not build:
        # Un modelo gguf SIN [download] y SIN [build] es SÓLO-LOCAL: el GGUF
        # tiene que estar ya en disco, ropero ni lo baja ni lo fabrica. Se
        # AVISA en vez de romper (spec §7.4: la obligación de declarar un
        # canal es del que quiere instalar el modelo; un catálogo entero no
        # se cae por un TOML con el [download] comentado a la espera de
        # decidir el canal).
        warn("sin [download] ni [build]: modelo gguf sólo-local "
             "(su GGUF debe estar ya en disco)")

    # ── [levels] ───────────────────────────────────────────────────────────
    levels_list = [str(x) for x in _as_list(levels.get("list"))]
    default = levels.get("default")
    if default is not None:
        if not levels_list:
            bad("[levels].default sin [levels].list")
        elif default not in levels_list:
            bad("[levels].default = '%s' no está en [levels].list (%s)"
                % (default, " ".join(levels_list)))

    # ── [flags] / [options] ────────────────────────────────────────────────
    flags_list = [str(x) for x in _as_list(flags.get("list"))]
    options_list = [str(x) for x in _as_list(options.get("list"))]
    options_default = loaded["options_default"]
    for key in options_default:
        if key not in options_list:
            bad("[options.default].%s no está declarado en [options].list" % key)

    # ── [aliases] ──────────────────────────────────────────────────────────
    for alias, level in aliases.items():
        if level is None or str(level) == "":
            bad("el alias '%s' no declara nivel" % alias)
        elif levels_list and str(level) not in levels_list:
            bad("el alias '%s' apunta al nivel '%s', que no está en [levels].list (%s)"
                % (alias, level, " ".join(levels_list)))

    # ── [limits] ───────────────────────────────────────────────────────────
    limits = loaded["limits"]
    if limits:
        if "min_ram_gb" in limits:
            value = limits.get("min_ram_gb")
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                bad("[limits].min_ram_gb tiene que ser numérico: %r" % (value,))
            elif value <= 0:
                bad("[limits].min_ram_gb tiene que ser > 0: %r" % (value,))

    # ── [args] ─────────────────────────────────────────────────────────────
    engine = model.get("engine") if model else None
    if engine == "llama-server":
        if not args_spec:
            bad("falta la tabla [args] (obligatoria para llama-server)")
        elif "base" not in args_spec:
            bad("falta [args].base")
        elif not isinstance(args_spec.get("base"), list):
            bad("[args].base no es una lista")
    elif engine and engine != "llama-server":
        # Un modelo con servidor propio puede no llevar flags, pero casi
        # siempre es un olvido de la conversión: se avisa (no rompe).
        base = args_spec.get("base") if args_spec else None
        if not base:
            warn("aviso: [args].base está vacío en un modelo de engine '%s' "
                 "(flags del servidor)" % engine)
    if args_spec:
        if "base" in args_spec and not isinstance(args_spec.get("base"), list):
            bad("[args].base no es una lista")
        for level, cfg in (args_spec.get("level") or {}).items():
            if levels_list and level not in levels_list:
                bad("[args.level.%s] no está declarado en [levels].list" % level)
            if not isinstance(cfg, dict):
                bad("[args.level.%s] no es una tabla" % level)
                continue
            if "base" in cfg and not isinstance(cfg.get("base"), list):
                bad("[args.level.%s].base no es una lista" % level)
            if "chat_template_kwargs" in cfg and not isinstance(
                    cfg.get("chat_template_kwargs"), dict):
                bad("[args.level.%s].chat_template_kwargs no es una tabla" % level)
            effort = cfg.get("reasoning_effort")
            if effort is not None and not isinstance(effort, str):
                bad("[args.level.%s].reasoning_effort no es una cadena" % level)
        for flag, cfg in (args_spec.get("flag") or {}).items():
            if flags_list and flag not in flags_list:
                bad("[args.flag.%s] no está declarado en [flags].list" % flag)
            if not isinstance(cfg, dict):
                bad("[args.flag.%s] no es una tabla" % flag)
                continue
            if "base" in cfg and not isinstance(cfg.get("base"), list):
                bad("[args.flag.%s].base no es una lista" % flag)
            for required in _as_list(cfg.get("requires")):
                if required not in FILE_KEYS:
                    bad("[args.flag.%s].requires referencia '%s', que no es un "
                        "fichero de [files] (%s)"
                        % (flag, required, ", ".join(FILE_KEYS)))
                elif not files.get(required):
                    bad("[args.flag.%s].requires referencia '%s', pero [files].%s "
                        "no está declarado" % (flag, required, required))
        for option, cfg in (args_spec.get("option") or {}).items():
            if options_list and option not in options_list:
                bad("[args.option.%s] no está declarado en [options].list" % option)
            if not isinstance(cfg, dict):
                bad("[args.option.%s] no es una tabla" % option)
                continue
            if "base" in cfg and not isinstance(cfg.get("base"), list):
                bad("[args.option.%s].base no es una lista" % option)

    # ── placeholders que apuntan a ficheros que no existen ────────────────
    def check_placeholders(items, where):
        for item in _as_list(items):
            if not isinstance(item, str):
                continue
            for key in FILE_KEYS:
                if "${%s}" % key in item and not files.get(key):
                    bad("%s usa ${%s} pero [files].%s no está declarado"
                        % (where, key, key))

    check_placeholders(args_spec.get("base"), "[args].base")
    for level, cfg in (args_spec.get("level") or {}).items():
        if isinstance(cfg, dict):
            check_placeholders(cfg.get("base"), "[args.level.%s].base" % level)
    for flag, cfg in (args_spec.get("flag") or {}).items():
        if isinstance(cfg, dict):
            check_placeholders(cfg.get("base"), "[args.flag.%s].base" % flag)
    for option, cfg in (args_spec.get("option") or {}).items():
        if isinstance(cfg, dict):
            check_placeholders(cfg.get("base"), "[args.option.%s].base" % option)

    # ── coherencia con la carpeta ──────────────────────────────────────────
    if engine_dir not in KNOWN_ENGINE_DIRS:
        bad("la carpeta '%s' no es un engine_dir conocido (%s)"
            % (engine_dir, ", ".join(KNOWN_ENGINE_DIRS)))

    return problems, warnings


def cmd_validate(opts):
    entries = iter_model_files(opts.root, include_fired=True)
    problems = []
    warnings = []
    seen = {}
    # Conjunto global de nombres de modelo (todas las origins) y aliases.
    all_model_names = set()
    all_aliases = {}
    for path, engine_dir, origin in entries:
        found, warned = validate_model(path, engine_dir, origin)
        problems.extend(found)
        warnings.extend(warned)
        try:
            loaded = load_model(path)
        except ModelConfError:
            continue
        name = loaded["model"].get("name")
        if not name:
            continue
        key = (origin, name)
        if key in seen:
            problems.append("%s: el nombre '%s' ya lo usa %s"
                            % (os.path.basename(path), name, seen[key]))
        else:
            seen[key] = os.path.basename(path)
        # El nombre del modelo no puede duplicar un alias ya registrado.
        if name in all_aliases:
            problems.append("%s: el nombre '%s' ya lo usa un alias en %s"
                            % (os.path.basename(path), name, all_aliases[name] if name in all_aliases else "?"))
        # Aliases: no pueden duplicar nombres de modelos ni otros aliases.
        aliases = loaded.get("aliases") or {}
        for alias in aliases:
            if alias in all_aliases:
                problems.append("%s: el alias '%s' ya lo usa %s"
                                % (os.path.basename(path), alias, all_aliases[alias]))
            elif alias in all_model_names:
                problems.append("%s: el alias '%s' duplica un nombre de modelo existente"
                                % (os.path.basename(path), alias))
            else:
                all_aliases[alias] = os.path.basename(path)
        all_model_names.add(name)

    for warning in warnings:
        sys.stderr.write("%s\n" % warning)
    for problem in problems:
        sys.stderr.write("%s\n" % problem)
    if problems:
        sys.stderr.write("%d problema(s) en %s\n" % (len(problems), opts.root))
        return 1
    sys.stdout.write("OK: %d modelo(s) validados en %s\n" % (len(entries), opts.root))
    if warnings:
        sys.stdout.write("%d aviso(s)\n" % len(warnings))
    return 0


# ===========================================================================
# CLI
# ===========================================================================

def build_parser():
    parser = argparse.ArgumentParser(
        prog="modelconf.py",
        description="Cargador de modelos de ropero (lee los .toml de models/).")
    subs = parser.add_subparsers(dest="command")

    dump = subs.add_parser("dump", help="volca todos los modelos (un registro por línea)")
    dump.add_argument("--root", required=True, help="directorio de modelos")
    dump.add_argument("--include-fired", action="store_true",
                      help="incluye también los modelos retired")
    dump.set_defaults(func=cmd_dump)

    args_cmd = subs.add_parser("args", help="imprime el argv final de un modelo")
    args_cmd.add_argument("--root", required=True, help="directorio de modelos")
    args_cmd.add_argument("--name", required=True, help="nombre del modelo")
    args_cmd.add_argument("--level", default=None, help="nivel de razonamiento")
    args_cmd.add_argument("--flag", action="append", default=[],
                          help="flag booleano a activar (repetible)")
    args_cmd.add_argument("--option", action="append", default=[],
                          help="opción con valor, k=v (repetible)")
    args_cmd.set_defaults(func=cmd_args)

    validate = subs.add_parser("validate", help="comprueba el esquema de cada .toml")
    validate.add_argument("--root", required=True, help="directorio de modelos")
    validate.set_defaults(func=cmd_validate)

    build_cmd = subs.add_parser(
        "build", help="imprime cómo se fabrica el GGUF de un modelo (su [build])")
    build_cmd.add_argument("--root", required=True, help="directorio de modelos")
    build_cmd.add_argument("--name", required=True, help="nombre del modelo")
    build_cmd.add_argument("--format", default="argv", choices=("argv", "json"),
                           help="argv: una opción por línea; json: el spec entero")
    build_cmd.add_argument("--out", default=None,
                           help="ruta EXACTA del GGUF final (la pone la instalación)")
    build_cmd.set_defaults(func=cmd_build)

    aux_cmd = subs.add_parser(
        "aux", help="imprime los ficheros extra de un modelo (mtp, imatrix, ...)")
    aux_cmd.add_argument("--root", required=True, help="directorio de modelos")
    aux_cmd.add_argument("--name", required=True, help="nombre del modelo")
    aux_cmd.set_defaults(func=cmd_aux)

    return parser


def main(argv):
    parser = build_parser()
    opts = parser.parse_args(argv)
    if not getattr(opts, "func", None):
        parser.print_help(sys.stderr)
        return 2
    try:
        return opts.func(opts)
    except ModelConfError as exc:
        sys.stderr.write("ERROR: %s\n" % exc)
        return 1
    except TomlError as exc:
        sys.stderr.write("ERROR: %s\n" % exc)
        return 1
    except BrokenPipeError:
        return 0
    except Exception as exc:  # noqa: BLE001 - la CLI no debe soltar tracebacks
        sys.stderr.write("ERROR: %s: %s\n" % (type(exc).__name__, exc))
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
