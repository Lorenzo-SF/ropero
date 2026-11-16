# ADR-012 · Configuración de rutas y presentación por engine

**Estado**: aceptada para Ropero v2  
**Fecha**: 2026-11-16

## Contexto

ADR-008 fijó una configuración global con `models_dir`, `gguf_dir`,
`safetensors_dir`, `bin_dir` y `logs_dir`. El usuario necesita ajustar por
engine dónde guarda sus TOMLs y artefactos, el rango de puertos y el color.
Los engines soportados forman un conjunto controlado: sus recetas de
instalación, dependencias y hooks no deben convertirse en configuración del
usuario.

El código actual refleja esa separación: `ropero-core::Config` contiene las
cinco rutas globales (`crates/ropero-core/src/config.rs`), los manifiestos
declaran metadatos fijos como color, puertos, dependencias y receta
(`crates/ropero-core/src/manifest.rs`), y `resolve_paths` deriva los artefactos
de GGUF y safetensors (`crates/ropero-core/src/paths.rs`).

## Decisión

1. **Se conservan los campos globales de ADR-008.** Los archivos existentes
   siguen siendo válidos. Los campos globales continúan siendo la base y el
   fallback para los engines sin overrides.

2. **Se admiten overrides por engine** bajo `[engines.<id>]`. Los campos son
   opcionales:

   ```toml
   [engines.ignis]
   models_toml_dir = "~/.config/ropero/models/ignis"
   artifacts_dir = "~/models/ignis"
   port_min = 9910
   port_max = 9919
   color = "cian"
   ```

   - `models_toml_dir` es el directorio que contiene los TOMLs de ese engine;
     por defecto deriva de `models_dir/<id>`.
   - `artifacts_dir` es la base para artefactos gestionados por el propio
     motor, como los `.ninfer` de Ignis. Si se omite, cae en `gguf_dir`.
   - `gguf_dir` y `safetensors_dir` son bases para los artefactos de ese
     formato. Se mantiene la organización por modelo bajo cada base, de modo
     que dos modelos del mismo engine no se mezclen. Si se omiten, se usan las
     rutas globales correspondientes.
   - `port_min` y `port_max` sustituyen el rango del manifiesto para ese engine.
   - `color` sustituye su color de manifiesto.

3. **Precedencia de rutas**: variable de entorno global > valor por-engine en
   `config.toml` > campo global en `config.toml` > valor por defecto. Las
   variables globales existentes mantienen su función: `ROPERO_MODELS_DIR` y
   `ROPERO_ROOT` para el catálogo, `GGUF_DIR` y `SAFETENSORS_DIR` para los
   artefactos. No se añaden variables por-engine en esta decisión. Las rutas
   relativas y `~` conservan la expansión existente.

4. **Precedencia de color y puertos**: override por-engine > valores del
   manifiesto del engine. No existe un override global para esos campos.

5. **Colores cerrados**: `color` sólo acepta los valores de `EngineColor` que
   ya usa la leyenda (`naranja`, `amarillo`, `rojo`, `gris`, `blanco`,
   `verde-pistacho`, `morado`, `verde-pálido`, `cian` y `sin-color`). No acepta códigos
   ANSI, secuencias de escape ni colores arbitrarios. Esto permite reasignar
   los colores existentes sin convertir el terminal en un canal de texto
   ejecutable.

6. **Rangos validados tras el overlay**: se comprueba el rango efectivo de
   cada engine después de aplicar `[engines.<id>]`. Los límites deben ser
   puertos válidos y `port_min <= port_max`; el conjunto efectivo también se
   valida para detectar rangos solapados. Un error se informa como error de
   configuración antes de usar los puertos.

7. **Configuración operativa cerrada**: no se exponen en `config.toml`
   repositorios de instalación, dependencias, gestores de paquetes, comandos
   pre/post instalación, flags de compilación ni hooks. Esos datos permanecen
   en los manifests y en el código controlado por Ropero. La configuración
   puede cambiar ubicación, puertos y color; no cambia qué engine instala
   Ropero ni cómo lo instala.

8. **Manifiestos centrales**: los manifests de engine siguen viviendo en el
   directorio `engines/` del catálogo/configuración descrito por ADR-008. La
   configuración por-engine sólo añade directorios de TOMLs de modelos; el
   descubrimiento mezcla esos TOMLs con el catálogo global y no mueve ni
   reescribe los manifests de engine.

## Compatibilidad y migración

- No se requiere migración para usuarios actuales: sin `[engines.<id>]`, cada
  engine conserva las rutas globales y los valores de su manifest.
- La configuración inicial mantiene los cinco campos globales de ADR-008.
  Las secciones por-engine son opt-in y pueden documentarse como ejemplos
  comentados.
- Las variables globales existentes conservan prioridad y semántica. En
  particular, `ROPERO_MODELS_DIR` continúa aceptando los layouts actuales de
  catálogo; no se redefine como un directorio obligatorio por-engine.
- Los manifiestos existentes siguen siendo fuente de verdad para instalación,
  dependencias y comportamiento. Un engine ausente de `EngineId` no se puede
  añadir mediante TOML.

## Consecuencias y límites

- El parser de configuración debe distinguir las cinco rutas raíz de las
  tablas `[engines.<id>]`, validarlas de forma estricta y conservar el origen
  de cada valor para `ropero config`.
- El runtime debe escanear cada `models_toml_dir` configurado y combinar los
  modelos con el catálogo normal, detectando IDs duplicados y manifiestos
  colocados bajo el engine incorrecto.
- El overlay de color y puertos debe aplicarse una sola vez antes de construir
  snapshots, listados, menús, doctor y planes de arranque. Todas esas vistas
  deben mostrar y usar los mismos valores efectivos.
- Compartir un directorio de artefactos no concede propiedad exclusiva de
  sus contenidos. La deduplicación de ficheros idénticos, el aislamiento de
  auxiliares y la política de `remove` son contratos separados; esta ADR no
  decide cómo resolverlos.
- `models_toml_dir` no añade escritura o edición automática de TOMLs. `ropero
  template` sigue imprimiendo la plantilla y sólo sugiere el destino.

## Orden de integración

1. Extender `ropero-core::Config` y su parser con la tabla tipada por
   `EngineId`, defaults, origen y validación.
2. Hacer que `Environment` y `resolve_paths` resuelvan rutas por engine con la
   precedencia acordada.
3. Extender el escaneo y bootstrap del runtime para los directorios de TOMLs
   por engine y aplicar los overrides de metadatos a los manifests efectivos.
4. Mostrar las rutas y sus orígenes en `ropero config`/`doctor`, y actualizar
   la plantilla de configuración y la especificación CLI.
5. Cubrir precedencia, compatibilidad de configuraciones antiguas, escaneo
   multi-directorio, duplicados, rangos solapados y consistencia de color y
   puertos en pruebas.
