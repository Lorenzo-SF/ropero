#!/usr/bin/env python3
"""
Genera el model card de un GGUF fabricado por 'ropero gguffer'.

  model-card.py --gguf FICHERO [--fuente FICHERO.json]
                [--readme SALIDA.md] [--condiciones SALIDA.md]
                [--usuario NOMBRE] [--hf-enlace URL]

Escribe DOS ficheros:

  README.md         la nota de "esto es una cuantizacion de terceros, no soy
                    el autor del modelo", arriba del todo, y un resumen.
  CUANTIZATION.md   las CONDICIONES de la cuantizacion: de que modelo salio,
                    con que quant, con que importance matrix, y que bloques
                    quedaron fuera de ella.

Por que dos: el README es lo que se lee en la ficha del repo, y tiene que
dejar claro que el modelo NO es mio. Las condiciones son el detalle, y que
no estorben al que solo quiere el fichero.

De donde sale cada dato:

  · el GGUF            -> el modelo base (general.base_model.0.repo_url), el
                           quant (general.file_type), la arch, los bloques.
  · el .fuente.json    -> lo que solo sabe el gguffer: de que repo se
                           bajaron los pesos, que importance matrix se uso,
                           y si la cabeza MTP quedo en un quant aparte.

Si el .fuente.json no esta, el card se hace igual, pero avisa de que la
procedencia no se conoce: es mejor decirlo que inventarla.
"""

import argparse
import json
import os
import sys

# El parser de GGUF vive en bin/, al lado de los demas scripts del repo.
_AQUI = os.path.dirname(os.path.abspath(__file__))
_BIN = os.path.normpath(os.path.join(_AQUI, "..", "bin"))
if _BIN not in sys.path:
    sys.path.insert(0, _BIN)

try:
    from _ggufcheck import Gguf, GgufError
except ImportError:                                     # pragma: no cover
    print("[x] no se encuentra bin/_ggufcheck.py", file=sys.stderr)
    sys.exit(2)


def _leer_fuente(ruta):
    """El .fuente.json del gguffer, o {} si no esta."""
    if not ruta or not os.path.exists(ruta):
        return {}
    try:
        with open(ruta, "r", encoding="utf-8") as f:
            d = json.load(f)
        return d if isinstance(d, dict) else {}
    except (OSError, ValueError):
        return {}


def _repo_corto(repo):
    """'https://huggingface.co/owner/repo' -> 'owner/repo'.

    La metadata del GGUF lo trae como URL entera, y el frontmatter de
    HuggingFace quiere 'owner/repo'. Sin esto, el base_model del card es una
    URL dentro de una URL.
    """
    if not repo:
        return ""
    r = repo.strip().rstrip("/")
    for pre in ("https://huggingface.co/", "http://huggingface.co/", "huggingface.co/"):
        if r.startswith(pre):
            r = r[len(pre):]
            break
    partes = [p for p in r.split("/") if p]
    if len(partes) >= 2:
        return "/".join(partes[-2:])
    return r


def _tipos():
    """numero de ggml_type -> nombre del quant.

    La lista esta en tipos.txt, que es la misma que lee quant-tipos.sh: el
    GGUF solo lleva el NUMERO (general.file_type), y en el card hace falta
    el nombre.
    """
    ruta = os.path.join(_AQUI, "tipos.txt")
    mapa = {}
    try:
        with open(ruta, "r", encoding="utf-8") as f:
            for linea in f:
                linea = linea.strip()
                if not linea or linea.startswith("#"):
                    continue
                partes = linea.split()
                if len(partes) == 2 and partes[0].isdigit():
                    mapa[int(partes[0])] = partes[1]
    except OSError:
        pass
    return mapa


def _nombre_quant(datos, fuente):
    """El nombre del quant: 'Q2_K', no el numero 10."""
    # El gguffer sabe el nombre: lo pide al cuantizador.
    q = fuente.get("quant") or ""
    if q and not q.isdigit():
        return q
    ft = datos.get("file_type")
    if isinstance(ft, int):
        q = _tipos().get(ft, "")
        if q:
            return q
    if q:
        return q
    # Ultimo recurso: el sufijo del nombre del fichero.
    nombre = fuente.get("gguf") or ""
    if "." in nombre:
        return nombre.rsplit(".", 1)[1]
    return ""


def _lee_gguf(ruta):
    """La metadata del GGUF. Solo la cabecera: un GGUF son 10 GB."""
    try:
        g = Gguf(ruta, solo_cabecera=True)
        return g.resumen()
    except (GgufError, OSError, ValueError) as e:
        print("[x] no se puede leer el GGUF %s: %s" % (ruta, e), file=sys.stderr)
        return {}


def _enlace_hf(repo):
    if not repo:
        return ""
    return "https://huggingface.co/%s" % repo


def _autor_de(repo):
    """El autor de un repo de HF es lo que va antes de la barra."""
    if not repo:
        return ""
    return repo.split("/")[0]


def _n_bytes(n):
    if not n:
        return "?"
    for u in ("B", "KiB", "MiB", "GiB", "TiB"):
        if n < 1024 or u == "TiB":
            return "%.1f %s" % (n, u) if u != "B" else "%d B" % n
        n /= 1024.0
    return "?"


def _readme(datos, fuente, usuario, hf_enlace):
    """El README del repo de HuggingFace."""
    base_repo = _repo_corto(datos.get("base_model_repo") or fuente.get("base_model") or "")
    base_name = datos.get("base_model_name") or fuente.get("base_model_name") or ""
    quant = _nombre_quant(datos, fuente)
    autor = _autor_de(base_repo)
    enlace = _enlace_hf(base_repo)
    fichero = fuente.get("gguf") or os.path.basename(datos.get("fichero", ""))

    L = []
    L.append("---")
    L.append("base_model: %s" % (base_repo or "DESCONOCIDO"))
    L.append("library_name: llama-cpp")
    L.append("tags:")
    L.append("  - gguf")
    if quant:
        L.append("  - %s" % quant)
    if base_repo:
        L.append("  - base_model:%s" % base_repo)
    L.append("---")
    L.append("")
    L.append("# %s%s" % (base_name or base_repo or "GGUF",
                         (" — %s" % quant) if quant else ""))
    L.append("")
    L.append("> [!WARNING]")
    L.append("> **Este repositorio no contiene el modelo: contiene una cuantizacion")
    L.append("> hecha por un tercero. No soy el autor del modelo.**")
    L.append(">")
    if base_repo:
        L.append("> - **Modelo original**: [`%s`](%s)" % (base_repo, enlace))
        if autor:
            L.append("> - **Autor del modelo**: %s (la licencia y los terminos de uso son" % autor)
            L.append(">   los suyos, no los mios: mira su ficha antes de usarlo)")
    if usuario:
        L.append("> - **Quien cuantiza**: %s, sin afiliacion con el autor original" % usuario)
    L.append("> - **Que es exactamente**: los mismos pesos del modelo original,")
    L.append(">   convertidos a GGUF y cuantizados a `%s`." % (quant or "un quant de bajo bit"))
    L.append(">")
    L.append("> Si el autor original publica su propio GGUF, **usa ese**: es el que")
    L.append("> el ha verificado. Este es un intento propio, sin garantia de nada.")
    L.append("")
    L.append("## Que hay en este repo")
    L.append("")
    L.append("| Fichero | Que es |")
    L.append("|---|---|")
    L.append("| `%s` | el modelo en GGUF, cuantizado a `%s` |" % (
        fichero or "modelo.gguf", quant or "?"))
    L.append("| `CUANTIZATION.md` | las condiciones exactas de la cuantizacion |")
    if fuente.get("imatrix"):
        L.append("| `%s` | el importance matrix usado, por si quieres refinar tus |" % (
            fuente["imatrix"].get("fichero", "imatrix.gguf")))
        L.append("|  | propias cuantizaciones con la misma matriz |")
    L.append("")
    L.append("## Como se usa")
    L.append("")
    L.append("Con [llama.cpp](https://github.com/ggml-org/llama.cpp):")
    L.append("")
    L.append("```bash")
    L.append("llama-server -m %s --jinja -ngl 99" % (fichero or "modelo.gguf"))
    L.append("```")
    L.append("")
    L.append("O con cualquier cliente que hable la API de llama.cpp")
    L.append("(`llama-cpp-python`, `LM Studio`, `Jan`, `KoboldCpp`...).")
    L.append("")
    L.append("## Detalles de la cuantizacion")
    L.append("")
    L.append("En `CUANTIZATION.md` esta todo: el importance matrix, el corpus, y")
    L.append("que bloques quedan fuera de la matriz.")
    L.append("")
    L.append("---")
    L.append("")
    L.append("Si ves un error o una cuantizacion que se puede mejorar, abre un issue")
    L.append("o un PR. Las correcciones son bienvenidas.")
    L.append("")
    return "\n".join(L)


def _condiciones(datos, fuente, usuario, hf_enlace):
    """CUANTIZATION.md: las condiciones, con detalle."""
    base_repo = _repo_corto(datos.get("base_model_repo") or fuente.get("base_model") or "")
    base_name = datos.get("base_model_name") or fuente.get("base_model_name") or ""
    quant = _nombre_quant(datos, fuente)
    imx = fuente.get("imatrix") or {}
    mtp = fuente.get("mtp") or {}

    L = []
    L.append("# Condiciones de la cuantizacion")
    L.append("")
    L.append("Este fichero es el registro de COMO se hizo este GGUF. Sin el, un")
    L.append("GGUF es un fichero opaco.")
    L.append("")
    L.append("## El modelo")
    L.append("")
    L.append("| | |")
    L.append("|---|---|")
    if base_repo:
        L.append("| Modelo | [`%s`](%s) |" % (base_repo, _enlace_hf(base_repo)))
    else:
        L.append("| Modelo | **desconocido** (el GGUF no lo dice) |")
    if base_name:
        L.append("| Nombre | %s |" % base_name)
    if fuente.get("fuente_repo"):
        L.append("| Repositorio de los pesos | `%s` |" % fuente["fuente_repo"])
    if datos.get("arch"):
        L.append("| Arquitectura | `%s` |" % datos["arch"])
    if datos.get("block_count"):
        L.append("| Bloques | %s |" % datos["block_count"])
    if datos.get("bytes"):
        L.append("| Tamano del GGUF | %s |" % _n_bytes(datos["bytes"]))
    L.append("")
    L.append("El modelo es de otro autor. Esta cuantizacion es mia, el modelo no.")
    L.append("")
    L.append("## La cuantizacion")
    L.append("")
    L.append("| | |")
    L.append("|---|---|")
    L.append("| Quant | `%s` |" % (quant or "?"))
    L.append("| Intermedio | `%s` (los pesos sin cuantizar, convertidos a GGUF) |" % (
        fuente.get("intermedio") or "F16"))
    L.append("| Herramienta | %s |" % (
        fuente.get("herramienta") or "llama-quantize de llama.cpp"))
    if fuente.get("fecha"):
        L.append("| Fecha | %s |" % fuente["fecha"])
    L.append("")
    L.append("## El importance matrix")
    L.append("")
    if imx:
        L.append("| | |")
        L.append("|---|---|")
        L.append("| Fichero | `%s` |" % imx.get("fichero", "?"))
        if imx.get("repo"):
            L.append("| Procede de | [`%s`](%s) |" % (imx["repo"], _enlace_hf(imx["repo"])))
        if imx.get("sha256"):
            L.append("| sha256 | `%s` |" % imx["sha256"])
        L.append("")
        L.append("La matriz va en este repo para que puedas refinar tus propias")
        L.append("cuantizaciones con la misma y comparar resultados.")
        L.append("")
        L.append("**De donde sale la matriz**: se calcula con `llama-imatrix` sobre el")
        L.append("modelo base, recorriendo un corpus con el que se mide la")
        L.append("importancia de cada tensor (cual de los canales se usa de verdad y")
        L.append("cual es ruido). Los tensores que no aparecen en la matriz se")
        L.append("cuantizan sin ella, con su tipo por defecto.")
    else:
        L.append("**Sin importance matrix**: los tensores se cuantizan con su tipo por")
        L.append("defecto, sin saber cuales son los importantes. Suele dar peor calidad")
        L.append("que con matriz, y este GGUF se hizo asi porque no habia una")
        L.append("disponible para este modelo.")
    L.append("")
    if mtp:
        L.append("## La cabeza MTP")
        L.append("")
        L.append("Este modelo lleva una capa de prediccion de la siguiente capa (MTP).")
        L.append("")
        L.append("| | |")
        L.append("|---|---|")
        L.append("| Bloque | `%s` (el ultimo) |" % mtp.get("bloque"))
        L.append("| Quant de ese bloque | `%s` |" % mtp.get("quant"))
        L.append("")
        L.append("**Por que no lleva el quant del resto**: el importance matrix se")
        L.append("calcula sobre el modelo base, y la cabeza MTP no esta en el")
        L.append("modelo base: no hay forma de medirla. Cuantizarla con un quant muy")
        L.append("bajo sin matriz degrada el modelo, asi que ese bloque se queda en")
        L.append("`%s`, que no la necesita." % mtp.get("quant"))
        L.append("")
        L.append("Es una decision consciente, no un descuido: el bloque MTP es una")
        L.append("cabeza extra, y a cambio de dejarla en un quant alto se pierde la")
        L.append("especulacion (que es donde se nota el rendimiento).")
    L.append("")
    L.append("## Reproducir esta cuantizacion")
    L.append("")
    L.append("```bash")
    L.append("ropero gguffer \\")
    if fuente.get("fuente_repo"):
        L.append("  --repo %s \\" % fuente["fuente_repo"])
    if imx.get("repo") and imx.get("fichero"):
        L.append("  --imatrix-repo %s \\" % imx["repo"])
        L.append("  --imatrix-file %s \\" % imx["fichero"])
    L.append("  --quant %s" % (quant or "QUANT"))
    L.append("```")
    L.append("")
    if usuario:
        L.append("---")
        L.append("")
        L.append("Hecho por %s. Cuantizacion de un modelo de otro autor: la" % usuario)
        L.append("licencia, las condiciones de uso y la responsabilidad del modelo")
        L.append("son del autor original.")
        L.append("")
    return "\n".join(L)


def main(argv=None):
    ap = argparse.ArgumentParser(add_help=True,
                                 description="genera el model card de un GGUF")
    ap.add_argument("--gguf", required=True, help="el GGUF")
    ap.add_argument("--fuente", default="", help="el .fuente.json del gguffer")
    ap.add_argument("--readme", default="", help="donde escribir el README.md")
    ap.add_argument("--condiciones", default="", help="donde escribir el CUANTIZATION.md")
    ap.add_argument("--usuario", default=os.environ.get("HF_USER", ""),
                    help="quien cuantiza (el usuario de HF)")
    ap.add_argument("--hf-enlace", default="", help="la URL del repo de destino")
    ap.add_argument("--solo-condiciones", action="store_true")
    args = ap.parse_args(argv)

    fuente_ruta = args.fuente
    if not fuente_ruta:
        fuente_ruta = args.gguf + ".fuente.json"
    fuente = _leer_fuente(fuente_ruta)
    datos = _lee_gguf(args.gguf)

    if not datos and not fuente:
        print("[x] no hay ni GGUF legible ni procedencia: no se puede escribir el card",
              file=sys.stderr)
        return 1

    if not fuente:
        print("[x] aviso: no hay procedencia (%s): el card no dira COMO se cuantizo"
              % fuente_ruta, file=sys.stderr)

    if args.readme and not args.solo_condiciones:
        with open(args.readme, "w", encoding="utf-8") as f:
            f.write(_readme(datos, fuente, args.usuario, args.hf_enlace))
    if args.condiciones:
        with open(args.condiciones, "w", encoding="utf-8") as f:
            f.write(_condiciones(datos, fuente, args.usuario, args.hf_enlace))
    return 0


if __name__ == "__main__":
    sys.exit(main())
