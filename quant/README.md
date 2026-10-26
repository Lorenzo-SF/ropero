# quant — fabricar, verificar y subir cuantizaciones

Todo lo de fabricar GGUFs con `ropero gguffer`, en un sitio y con un guion.

## Que hay aqui

| Fichero | Que hace |
|---|---|
| `modelos.tsv` | **La tabla de QUE fabricar.** La fuente de verdad. |
| `cuantizar-todos.sh` | Cuantiza segun la tabla (varios quants por tirada). |
| `verificar-gguf.sh` | Comprueba un GGUF sin necesidad de GPU. |
| `subir-hf.sh` | Sube a HuggingFace lo que no tenga el marcador `.hf-uploaded`. |
| `todo.sh` | El guion completo: cuantizar → verificar → subir. |
| `hf.env` | Tu usuario de HuggingFace. |
| `lista-para-generar.md` | La lista de los 8 quants que compensa fabricar, con el porqué. |
| `que-compensa-cuantizar.md` | El criterio (el filtro de VRAM) y lo que ya existe en HF. |
| `comandos-cuantizar.md` | Los comandos a mano, para una tirada suelta. |

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
