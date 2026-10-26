#!/usr/bin/env python3
"""Ayudante de probar-bucle.sh: arma el body, lee la tool y mete el resultado.

Se separa en un fichero aparte a proposito: los heredocs de bash anidados
dentro de $( ) son fragiles (un delimitor mal cerrado deja el "PY" executing
como si fuera un comando), y aqui no hay nada que se pueda colar.

Modos:
    body <historial> <tools-json>   -> escribe el body de la peticion
    tool                             -> lee la respuesta de stdin y dice que
                                       tool pide (o "-" si no pide ninguna)
    push <historial>                 -> mete la respuesta de stdin en el
                                       historial, con el resultado de la tool
"""
import json
import sys


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

    print("modo desconocido: %s" % modo, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
