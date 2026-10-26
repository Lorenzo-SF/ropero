#!/usr/bin/env python3
"""Ayudante de probar-bucle.sh y probar-sampling.sh.

Modos:
    body <historial> <tools-json>   -> el body de la peticion
    tool                             -> de la respuesta de stdin, que tool pide
                                      (o "-" si no pide ninguna)
    push <historial>                 -> mete la respuesta de stdin en el
                                      historial, con el resultado de la tool
    bucle <url> <turnos> <key>      -> el bucle entero. Imprime una linea por
                                      turno y, al final, un resumen JSON:
                                      {"llamadas":N,"misma":N,"tps":X,"rompe":bool}

Se separa en un fichero aparte a proposito: los heredocs de bash anidados
dentro de $( ) son fragiles (un delimitador mal cerrado deja el "PY"
ejecutandose como si fuera un comando), y aqui no hay nada que se pueda colar.
"""
import json
import sys
import time
import urllib.error
import urllib.request

TOOLS = [{
    "type": "function",
    "function": {
        "name": "vault_index",
        "description": "Indexa el vault y devuelve la lista de ficheros",
        "parameters": {
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"],
        },
    },
}]

MSG_USER = ("Antes de escribir codigo, mira que hay en el vault. "
            "Usa la tool vault_index.")


def _post(url, key, body, timeout=180):
    datos = json.dumps(body).encode()
    req = urllib.request.Request(url, data=datos, method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Bearer " + key)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())


def modo_bucle(argv):
    url = argv[0]
    turnos = int(argv[1])
    key = argv[2]

    historial = [{"role": "user", "content": MSG_USER}]
    llamadas = 0
    misma = 0
    ultima = ""
    tokens = 0
    segundos = 0.0

    for i in range(turnos):
        body = {
            "messages": historial,
            "tools": TOOLS,
            "max_tokens": 200,
            "temperature": 0.6,
            "top_p": 0.95,
            "stream": False,
        }
        t0 = time.time()
        try:
            resp = _post(url, key, body)
        except urllib.error.HTTPError as e:
            print("  turno %d: HTTP %d" % (i + 1, e.code))
            break
        except Exception as e:
            print("  turno %d: error %s" % (i + 1, e))
            break
        segundos += time.time() - t0

        usage = resp.get("usage") or {}
        tokens += int(usage.get("completion_tokens") or 0)

        msg = (resp.get("choices") or [{}])[0].get("message", {}) or {}
        calls = msg.get("tool_calls") or []
        tool = calls[0]["function"]["name"] if calls else "-"

        if tool == "-":
            print("  turno %d: responde en texto (sin tool) -> el bucle esta roto" % (i + 1))
            break

        llamadas += 1
        if tool == ultima:
            misma += 1
        print("  turno %d: pide la tool '%s'" % (i + 1, tool))
        ultima = tool

        historial.append(msg)
        for tc in calls:
            historial.append({
                "role": "tool",
                "tool_call_id": tc["id"],
                "content": "vault vacio: 1 fichero encontrado",
            })

    tps = round(tokens / segundos, 1) if segundos > 0 and tokens else 0
    # "rompe" = el bucle se ROMPE (o sea, el modelo deja de repetir).
    print(json.dumps({
        "llamadas": llamadas,
        "misma": misma,
        "tps": tps,
        "rompe": misma < 2,
    }))
    return 0


def main():
    modo = sys.argv[1] if len(sys.argv) > 1 else ""

    if modo == "body":
        historial = json.load(open(sys.argv[2], encoding="utf-8"))
        tools = [json.loads(sys.argv[3])]
        body = {
            "messages": historial,
            "tools": tools,
            "max_tokens": 200,
            "temperature": 0.6,
            "top_p": 0.95,
            "stream": False,
        }
        print(json.dumps(body))
        return 0

    if modo == "tool":
        try:
            d = json.load(sys.stdin)
        except Exception:
            print("?")
            return 0
        msg = d.get("choices", [{}])[0].get("message", {}) or {}
        calls = msg.get("tool_calls") or []
        print(calls[0]["function"]["name"] if calls else "-")
        return 0

    if modo == "push":
        historial = json.load(open(sys.argv[2], encoding="utf-8"))
        resp = json.load(sys.stdin)
        msg = resp["choices"][0]["message"]
        historial.append(msg)
        for tc in (msg.get("tool_calls") or []):
            historial.append({
                "role": "tool",
                "tool_call_id": tc["id"],
                "content": "vault vacio: 1 fichero encontrado",
            })
        json.dump(historial, open(sys.argv[2], "w", encoding="utf-8"))
        return 0

    if modo == "bucle":
        return modo_bucle(sys.argv[2:])

    print("modo desconocido: %s" % modo, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
