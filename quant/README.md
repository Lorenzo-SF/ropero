# quant — fabricar, verificar y subir cuantizaciones

Todo lo de fabricar GGUFs con `ropero gguffer`, en un sitio y con un guion.

## Que hay aqui

| Fichero | Que hace |
|---|---|
| `ACRITO.md` | **El procedimiento completo**, con los comandos exactos y los errores que salen. |
| `modelos.tsv` | **La tabla de QUE fabricar.** La fuente de verdad. |
| `cuantizar-todos.sh` | Cuantiza segun la tabla (varios quants por tirada). |
| `verificar-gguf.sh` | Comprueba un GGUF sin necesidad de GPU. |
| `subir-hf.sh` | Sube a HuggingFace lo que no tenga el marcador `.hf-uploaded`. |
| `todo.sh` | El guion completo: cuantizar → verificar → subir. |
| `hf.env` | Tu usuario de HuggingFace. |
| `lista-para-generar.md` | La lista de los 8 quants que compensa fabricar, con el porqué. |
| `que-compensa-cuantizar.md` | El criterio (el filtro de VRAM) y lo que ya existe en HF. |
| `comandos-cuantizar.md` | Los comandos a mano, para una tirada suelta. |
| `aplicar-parche-motor.sh` | Aplica al clon del motor los dos parches que necesita (la regla del NVFP4 y los patterns sin anclar). |
| `patches/` | Los parches del motor, como ficheros. |

## Los parches del motor

Dos cosas del motor (que es un fork de otro dueño) hay que parchearlas, y
no se pueden pushear: se entregan como ficheros y se aplican en local.

```bash
quant/aplicar-parche-motor.sh --check    # ¿estan puestos?
quant/aplicar-parche-motor.sh            # los pone y recompila
quant/aplicar-parche-motor.sh --revert   # vuelve atras
```

- **La regla del NVFP4** (`src/llama-quant.cpp`): el motor tiene una regla
  para el MXFP4_MOE (los tensores de expertos a 4 bits, el resto a Q8_0) y
  no la tiene para el NVFP4. Sin ella, el NVFP4 cuantiza a 4 bits tambien
  la atencion, las proyecciones del Mamba y el embedding, y el modelo
  "habla cosas raras". **Sin este parche, el NVFP4 del nemotron no sirve.**
- **Los patterns sin anclar** (`common/json-schema-to-grammar.cpp`): el
  autoparser de templates rechazaba los `pattern` de las tools que no
  empiezan por `^` y acaban en `$`, y devolvia un 400 a la peticion
  entera. Afecta a los modelos sin plantilla "specialized" (el granite, el
  qwen3).

## El guion

```bash
cd quant
echo "HF_USER=tu-usuario-de-huggingface" > hf.env

./todo.sh --listar        # lo que hay y lo que falta
./todo.sh                 # cuantiza, verifica y sube
./todo.sh fable           # solo ese modelo
./todo.sh --sin-subir     # cuantiza y verifica, pero no sube
./todo.sh --solo-subir    # no cuantiza: sube lo pendiente
```

## Por que esto y no scripts sueltos

- **La tabla es la fuente de verdad.** `modelos.tsv` dice qué fabricar; los
  scripts no llevan la lista dentro. Se edita la tabla, no el código.
- **Varios quants en una tirada.** Bajar y convertir los pesos es lo caro
  (40-60 GB y una hora); cuantizar es solo CPU. El gguffer convierte el GGUF
  intermedio UNA vez y lo reutiliza para todos los quants de la línea. El
  primero es el "principal" (va a la ruta que espera el TOML); los demás
  quedan en `$GGUF_DIR/_variantes/<modelo>/`.
- **La ruta la decide el TOML**, y se pide con `ropero path <modelo>`, no
  leyendo los TOMLs aparte: un lector propio se queda viejo en cuanto se
  renombra un modelo o se cambia su `[files] gguf`, y entonces el GGUF se
  fabrica donde nadie lo busca.
- **Se verifica antes de subir.** Un GGUF que no pasa la verificacion no se
  sube: es mejor no subir que subir un GGUF roto.
- **El marcador `.hf-uploaded`.** Junto al GGUF, en su carpeta, hay un
  fichero `.hf-uploaded` con el repo de HF donde se subio. Si existe, ese
  GGUF no se vuelve a subir. Los GGUFs de `$GGUF_DIR` son todos tuyos
  (cuantizados, no descargados), asi que todos son candidatos.

## El model card sale del propio GGUF

No hay que mantener una tabla de "este GGUF viene de este repo": el GGUF
lleva su partida en la metadata (`general.base_model.0.repo_url`), y el
quant en `general.file_type`. De ahi salen el nombre del repo de destino y
el `base_model` del model card.

## Ojo con el disco

Cada línea de la tabla descarga el repo de pesos entero (40-60 GB) y genera
un GGUF intermedio del mismo tamaño. **El pico son ~120 GB por modelo.** El
gguffer borra el clon y el intermedio cuando el GGUF final valida, y
reutiliza el clon si relanzas la misma línea.


## La fuente "gguf": cuantizar un GGUF que ya existe

A veces el conversor de pesos no sabe convertir el modelo. El caso real es
el **Qwen3.8-27B**: es multimodal y lleva cabeza MTP, y el conversor del
fork que usa ropero no sabe mapear los tensores de la MTP, asi que la
conversion aborta (y con `--no-mtp` tambien, porque esa clase ignora el
flag).

La salida es no convertir: hay gente que ya publica el GGUF sin cuantizar
(bartowski publica el BF16 del Qwen3.8), y ese GGUF ya tiene la MTP bien
metida. Se cuantiza ese:

```bash
ropero gguffer \
  --gguf bartowski/Qwen3.8-27B-GGUF:Qwen3.8-27B-bf16/Qwen3.8-27B-bf16-00001-of-00002.gguf \
  --quant MXFP4 \
  --imatrix-repo bartowski/Qwen3.8-27B-GGUF \
  --imatrix-file Qwen3.8-27B-imatrix.gguf
```

En la tabla, esto es la septima columna, **"fuente"**: vacia = clonar los
pesos y convertir; puesta = un GGUF que ya esta hecho. Si el GGUF va
partido en shards, se bajan todos y se unen solos.

**El fichero de entrada no se borra nunca**: es del usuario.

El detalle entero del caso, con las medidas, esta en **ACRITO.md**.
