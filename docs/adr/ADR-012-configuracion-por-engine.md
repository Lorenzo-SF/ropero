# ADR-012 · Rutas compartidas y presentación por engine

**Estado**: aceptada para Ropero v2  
**Fecha**: 2026-11-16

## Contexto

Ropero mantiene un catálogo TOML por engine y almacena artefactos en rutas
configurables por el usuario. Esas ubicaciones deben ser compartidas: duplicar
`gguf_dir`, `safetensors_dir` o `models_toml_dir` en cada engine provoca
configuraciones divergentes, rutas olvidadas y modelos que no aparecen. FLM,
además, gestiona internamente su almacén de modelos.

Cada engine sí puede necesitar puertos distintos y un color propio. Sus recetas,
dependencias y hooks siguen siendo propiedad de Ropero y no son configurables.

## Decisión

1. Todas las rutas se declaran juntas bajo `[models.paths]`:

   ```toml
   [models.paths]
   models_dir = "~/.config/ropero/models"
   gguf_dir = "~/models/gguf"
   safetensors_dir = "~/models/safetensors"
   artifacts_dir = "~/models/ignis"
   bin_dir = "~/.local/bin"
   logs_dir = "~/.config/ropero/logs"
   ```

2. El catálogo de cada engine está en `models_dir/<engine-id>/`. Los artefactos
   usan las rutas compartidas según su formato. FLM instala y lista sus modelos
   a través de su almacén propio; no necesita una ruta de artefactos en su
   sección de engine.

3. `[engines.<id>]` sólo permite `port_min`, `port_max`, `color` y `api_key`.
   Puertos y color sustituyen los valores del manifest; `api_key` se envía como
   Bearer al endpoint local. No se exponen por config rutas ni recetas de
   instalación, dependencias, comandos o hooks.

4. Se conservan los campos de ruta antiguos en `[engines.<id>]` durante la
   transición de configuración, pero Ropero los ignora y `doctor` los señala
   para migrarlos a `[models.paths]`. La plantilla nueva ya no los genera.

5. El escaneo prefiere la carpeta canónica del engine. Los nombres históricos
   `flm`, `mlx` y `tensorrt` sirven como fallback cuando el directorio canónico
   no contiene TOMLs. Si ambos tienen TOMLs, sólo se escanea el canónico, para
   no duplicar modelos.

6. Los overrides de color y puertos se validan después de aplicarlos al
   manifest. Los rangos deben ser válidos y no solaparse. Los colores pertenecen
   al conjunto cerrado de `EngineColor`; no se aceptan secuencias ANSI.

## Compatibilidad y migración

- Las rutas globales antiguas en la raíz de `config.toml` siguen leyéndose.
- Las rutas por engine antiguas se aceptan pero dejan de tener efecto; deben
  consolidarse bajo `[models.paths]`.
- Los TOMLs se mantienen en carpetas de engine canónicas. Si el directorio
  canónico está vacío y existe un directorio legacy con modelos, éste se usa.
- Las variables globales existentes mantienen prioridad sobre el archivo:
  `ROPERO_MODELS_DIR`, `GGUF_DIR`, `SAFETENSORS_DIR`, `ARTIFACTS_DIR`,
  `ROPERO_BIN_DIR` y `LOG_DIR`.

## Consecuencias

- `ropero config` y `ropero doctor` muestran las rutas compartidas y sólo los
  overrides de engine que tienen efecto.
- El runtime escanea una carpeta por engine y no mezcla catálogos duplicados.
- La ruta de compilación de un engine pertenece a su instalación bajo
  `~/.local/share/ropero/engines/<id>/build-ropero/`; no es una ruta de modelo
  ni se configura en `config.toml`.
