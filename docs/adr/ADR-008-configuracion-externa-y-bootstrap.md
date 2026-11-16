# ADR-008 · Configuración externa, bootstrap y catálogo de usuario

**Estado**: aceptada
**Fecha**: 2026-11-15

## Contexto

La v2 necesitaba pasar de un catálogo descubierto en el repositorio a un
modelo de uso normal basado en configuración de usuario, sin romper los
flujos de desarrollo y pruebas existentes. A la vez había que fijar dónde
viven por defecto los logs y cómo se accede a los manifiestos de motor sin
embutirlos como fallback interno.

## Decisión

1. **Catálogo normal externo**: en uso normal el catálogo de modelos vive en
   `<directorio_config>/models/<motor>/` y los manifiestos de motor en
   `<directorio_config>/engines/`. El repositorio `models/` y `engines/` es
   solo para desarrollo y pruebas y **no se auto-descubre** en uso normal.

2. **Overrides de entorno preservados**: `ROPERO_MODELS_DIR` y `ROPERO_ROOT`
   siguen funcionando como overrides. Si `ROPERO_MODELS_DIR` apunta al
   directorio `models/` de la raíz de un catálogo/repositorio, se conserva la
   semántica actual de resolución (incluida la búsqueda de manifiestos en la
   raíz hermana `<raíz>/engines/` cuando el escaneo parte de `models/`).

3. **Directorio y archivo de configuración**: `<directorio_config>` es
   `$XDG_CONFIG_HOME/ropero` o `~/.config/ropero`. El archivo es
   `<directorio_config>/config.toml`. En el primer uso se crea el directorio y
   el archivo por defecto de forma idempotente y sin sobrescribir
   (`create_new`).

4. **Precedencia**: `variable de entorno > archivo de configuración > valor
   por defecto` para `models_dir`, `gguf_dir`, `safetensors_dir`, `bin_dir` y
   `logs_dir`.

5. **Logs por defecto**: `logs_dir` por defecto es `<directorio_config>/logs`
   (no `~/.logs/ropero`).

6. **Manifiestos de motor**: no se embuten ni se usan manifiestos internos
   como fallback. El acceso es por directorio `engines/` junto a `models/` en
   la raíz del catálogo (patrón ya adoptado por el repositorio), de modo que
   el catálogo de usuario puede funcionar cuando dispone de sus manifiestos en
   `<directorio_config>/engines/`.

7. **Superficie CLI**: `ropero config` muestra las rutas resueltas y su
   origen; `ropero doctor` informa del estado de la configuración; `ropero
   template` imprime la plantilla de modelo y sugiere un destino en
   `models/` sin escribir en disco.

## Consecuencias

- El arranque/bootstrap crea la estructura mínima de configuración y catálogo
  externo sin interferir con configuraciones existentes.
- Los tests y flujos de desarrollo siguen pudiendo apuntar a catálogos del
  repositorio mediante variables de entorno.
- Un catálogo de usuario vacío es válido; para listar/ejecutar modelos y
  motores se espera que el usuario disponga de manifiestos y TOMLs en su
  directorio de configuración (o use overrides).