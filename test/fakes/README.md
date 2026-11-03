# Fakes para la suite de test de ropero

Estos binarios NO son de opencode, ni de ropero, ni de curl, ni de pkill: son
dobles de prueba (test doubles) que permiten ejercitar el runner completo
**sin GPU, sin red y sin tocar nada del usuario**.

## Para qué sirven

El runner de `test/lib/test-common.sh` hace, por batería:

1. `ropero stop all` → para lo que haya en el entorno.
2. `ropero <modelo> --background` → arranca el modelo y devuelve
   `ROPERO_PORT=<n>` como última línea de stdout.
3. `curl http://127.0.0.1:<puerto>/v1/models` → probe del engine.
4. `opencode run <prompt> --agent build --model ropero-local/<modelo>
   --format json --standalone` → la batería.
5. `opencode session export <sid> --standalone` → la sesión exportada.

Los fakes sustituyen a los cinco comandos externos y simulan su contrato:

| fake      | qué simula                                                        |
|-----------|-------------------------------------------------------------------|
| `ropero`  | el arranque en background y el `ROPERO_PORT=`; escribe un log de engine |
| `opencode`| un JSON con `sessionID` y la sesión exportada                     |
| `curl`    | el probe de `/v1/models` (200 con key, 401 sin key)                |
| `pkill`   | **no mata nada**: sólo registra lo que se le pide                  |

## Cómo se usan

```bash
HOME=$(mktemp -d) PATH="$PWD/test/fakes:$PATH" bash test/test-groot.sh
```

Con eso el runner deja en `$HOME/test/groot/`:

```
prueba1/  prueba2/  prueba3/  prueba_larga/  prueba_gorda/
recopilacion-completa.md
```

y cada batería con su `opencode.json` (provider hermético `ropero-local`),
su JSON, su sesión exportada, su log del engine y su stderr.

## Reglas

- **Nunca** pongas `test/fakes` en el PATH de forma permanente: el `curl` y
  el `pkill` falsos romperían tu trabajo normal.
- El `pkill` falso no mata nada a propósito: el real mataría la sesión de
  opencode del usuario.
- Cada fake registra sus invocaciones en `$HOME/.fake-<nombre>.log` (o en
  `$FAKE_LOG`): ahí se puede comprobar qué se llamó y con qué argumentos.
- `FAKE_ROPERO_PORT` cambia el puerto que anuncia el `ropero` falso
  (por defecto 9999).
