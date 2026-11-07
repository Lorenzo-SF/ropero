# Strata en ropero

[Strata](https://github.com/Niko1221/Strata) no es un servidor de inferencia
como `llama-server`: es un **Terminal** que baja un motor (el binario que
habla con la GPU), deja el modelo en disco y sirve una API OpenAI-compatible.
Su motor reparte los pesos de un MoE entre **VRAM, RAM y SSD**, y lo que no
cabe en la memoria se lee del disco (el modo *low-RAM*).

## Instalar

```bash
ropero install engines strata
```

Eso ejecuta el instalador oficial del proyecto (sin permisos de root; mete
todo en el HOME):

```bash
curl -fsSL https://github.com/Niko1221/Strata/releases/latest/download/install.sh \
    -o install.sh && sh install.sh
```

El instalador **no** trae el motor ni el modelo: los baja `strata launch` la
primera vez, del mismo release y con la firma comprobada.

Si prefieres el Strata de escritorio (o lo quieres junto al Terminal), eso es
otro camino: este engine habla con el Terminal.

## Los modelos

Los modelos de Strata **no son de ropero**: su catálogo es el de Strata, con
su `--family` y su `--model`. Un modelo de ropero con `engine = "strata"` es
un puntero:

```toml
[model]
name    = "strata-qwen38-q2"
engine  = "strata"
context = 32768
format  = "strata"
path    = "qwen/Q2_0"
```

`[model].path` es el par **familia/paquete** del catálogo de Strata (`qwen` +
`Q2_0`), no una ruta. Los archivos los baja y los prepara Strata en su
carpeta; ropero no los toca.

Los que trae el catálogo ahora mismo:

| modelo de ropero | familia/paquete | pesos | para qué |
|---|---|---|---|
| `strata-qwen36-q4` | `qwen36/UD-Q4_K_XL` | 22,9 GB | el que rinde: cabe en la RAM y va a **228 t/s** medidos en una RTX 5080 Laptop |
| `strata-qwen38-q2` | `qwen/Q2_0` | 66,4 GB | el modelo grande (125B) en la cuantización más pequeña; **no** cabe en 30 GB de RAM → modo low-RAM (2,9-3,2 t/s) |
| `strata-gyro-s` | `gyro/Gyro-S` | 58,5 GB | la variante que ahorra memoria (27,6 GiB de pesos + 28,8 GB de tabla n-gram dentro del fichero) |

## Arrancar

```bash
ropero use strata-qwen36-q4
```

Eso:

1. lanza `strata launch` con el par familia/paquete del modelo, el puerto
   interno de ropero y `--access local --no-api-key`;
2. **descarga el modelo** si no está (aquí es donde se va el tiempo: hasta
   72 GB). El arranque tiene un timeout de 300 s por defecto; para una
   descarga, súblelo:

   ```bash
   ROPERO_START_TIMEOUT=7200 ropero use strata-qwen38-q2
   ```

3. levanta el proxy de ropero por delante, que es el que expone el puerto y
   el que exige la API key (`LLAMA_API_KEY`).

El log del motor está en el logfile de ropero; `ropero status` dice si está
listo.

## Las tres piezas y por qué

**El proxy (`bin/server/endpoint.py --backend strata`).** Strata arranca con
`--access local --no-api-key`: sólo escucha en 127.0.0.1 y no comprueba la
clave. La clave la pone el proxy, que es el único puerto expuesto. El proxy
además reescribe el campo `model` al id de ropero, así que el cliente ve el
nombre que pidió.

**El puente (`bin/server/strata-run.sh`).** `strata launch --no-tui` arranca
el motor y **devuelve** cuando el server ya sirve (el motor sigue vivo, pero
lo gestiona Strata). El proxy necesita un proceso que no muera mientras el
modelo sirve y que muera cuando hay que pararlo: si no, el proxy daría el
backend por caído y el modelo seguiría ocupando la VRAM sin nadie que lo
supervise. El puente hace las dos cosas.

**El preset.** `strata launch --stop` (a secas) para *todos* los Strata que
haya corriendo, y el usuario puede tener el suyo. Por eso el launch se
guarda con `--save-preset ropero-<modelo>` y el puente para *ese* preset y
sólo ese.

## Lo que NO hace ropero

- **No baja los modelos.** No hay modo de "descargar sin arrancar" en Strata:
  el modelo se baja al arrancar el motor. `ropero install models <modelo>`
  enseña el **plan** (qué se bajaría, cuánto y si cabe en esta máquina) y lo
  dice, en vez de fingir que ha bajado nada.
- **No toca los ficheros de Strata.** Los pesos, la tabla n-gram, el motor y
  la carpeta de Strata son suyos. `ropero stop` para lo que arrancó ropero; lo
  que arrancó el usuario, no se toca.
- **No gestiona los presets de Strata.** Los que crea ropero se llaman
  `ropero-<modelo>`; se ven en las pantallas de Strata.

## Strata fuera de ropero

Si prefieres usar Strata por su cuenta (con su monitor y sus pantallas), se
conecta a las herramientas con su propio comando:

```bash
strata connect opencode --key-env OPENAI_API_KEY
```

Eso escribe la configuración de opencode con la dirección y la clave del
servidor de Strata. Es un camino **paralelo** al de ropero: si el modelo lo
sirve ropero, el cliente habla con el proxy de ropero (puerto de ropero, clave
de ropero) y no hace falta tocar la config de opencode.

## La trampa de la VRAM

Con menos de 512 MiB de VRAM libres, el driver de NVIDIA pagina y la petición
no vuelve nunca. Si Strata se queda sin VRAM:

```bash
ropero use strata-qwen36-q4 --vram-reserve 1344
```

(o, con Strata directamente: `strata launch ... --set vram_reserve_mib=1344`).
El valor de 1344 MiB es el medido en esta máquina; el motor lo llama
`--vram-reserve-mib`.
