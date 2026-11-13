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

## Los fakes del gguffer (`test/fakes/gguffer/`)

El gguffer habla con HuggingFace (por `curl` y por `hf`) y con el motor
(por `convert_hf_to_gguf.py` y `llama-quantize`). Para poder probarlo sin
red y sin el motor, hay un juego de dobles propio:

| fake                        | qué simula                                                        |
|-----------------------------|-------------------------------------------------------------------|
| `gguffer/curl`              | la metadata de los repos (`config.json`, `hf_quant_config.json`) y la API de modelos. Un 404 responde como el curl de verdad (código 22, sin escribir) |
| `gguffer/hf`                | `hf download`: clona el repo de pesos o baja el imatrix. Registra cada llamada en `$FAKE_HF_LOG` |
| `gguffer/gguf_fake.py`      | un GGUF sintético con el `general.file_type` que haga falta       |
| `gguffer/convert_hf_to_gguf.py` | el conversor de pesos: decide el `file_type` de salida por la metadata del repo (NVFP4 → 39, MXFP4 → 38, el resto → F16) |
| `gguffer/llama-quantize`    | el cuantizador: `--help` imprime la lista real de quants del motor; cuantizar copia el GGUF y le pone el `file_type` del quant pedido; **re-cuantizar falla** con el mensaje del motor ("requantizing from type X is disabled") |
| `gguffer/motor/`            | el `LLAMA_DIR` falso: el conversor y el `build/bin/llama-quantize` |

El `llama-quantize` falso registra cada cuantización en `$FAKE_QUANT_LOG`,
que es lo que permite comprobar que **no** se cuantiza lo que ya está en el
quant pedido.

La metadata de los repos de ejemplo vive en `$FAKE_HF_DIR/<owner>/<repo>/`
(`config.json` y/o `hf_quant_config.json`).

El test que los usa es `test/test-gguffer.sh`:

```bash
bash test/test-gguffer.sh
```
