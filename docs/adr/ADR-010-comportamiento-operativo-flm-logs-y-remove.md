# ADR-010 · Comportamiento operativo: FLM delegado, logs/uso y eliminación de modelos

**Estado**: aceptada
**Fecha**: 2026-11-16

## Contexto

La implementación actual incorpora comportamiento operativo no cubierto de forma
explícita por los ADR previos: cómo se integra FastFlowLM (FLM) como almacén
delegado de modelos, cómo se gestionan los logs y el registro de uso en
arranques, y cómo se eliminan artefactos de modelos de forma segura. Estos
aspectos afectan al contrato observable del CLI y a las garantías de seguridad,
pero no definen el umbral de líneas del CLI (cubierto por
[ADR-009](ADR-009-umbral-de-lineas-del-cli-tras-config.md)).

## Decisión

### 1. FLM como almacén delegado

- El identificador operativo de un modelo FLM es un **tag**: el primario es
  `[model].path` cuando no está vacío; en su defecto, el valor de
  `--decision` declarado en `[args].base`.
- La instalación, eliminación y consulta del almacén se **delegan** a los
  comandos declarados por el manifiesto del motor (`install.pull`,
  `install.remove`, `install.list`) cuando están presentes. No se asume un
  inventario propio gestionado por ropero.
- El arranque de modelos FLM delegados compone `flm serve <tag> ...` con los
  `defaults` del manifiesto y el argv del modelo. Si `--ctx-len` no está ya
  presente, puede añadirse a partir del contexto efectivo.
- La detección de estado mediante `flm list --json` es de **mejor esfuerzo**:
  una respuesta no concluyente o un fallo de consulta no se interpreta como
  prueba fuerte de presencia/ausencia. El contrato operativo no afirma
  verificación extremo a extremo de FLM.

### 2. Logs, colorizado y registro de uso

- En primer plano, ropero muestra la salida del motor en la terminal (con
  colorizado cuando procede) y el motor escribe además en su fichero de log
  habitual, de modo que el log sigue disponible para diagnóstico de fallos y
  estado.
- Con `--background`, el proceso se desacopla: no hay salida en la terminal ni
  emisión de `ROPERO_PORT`, y los logs se conservan bajo `logs_dir`.
- El colorizado combina reglas **comunes y semánticas** (errores, avisos,
  rendimiento, carga y actividad) con patrones característicos de motores
  conocidos. No es un parser completo por formato de log; lo que no reconoce
  recibe el tratamiento común seguro.
- Se mantiene un log de uso local **append-only** en `logs_dir/usage.log` con
  información mínima de operación (instante, modelo, motor, puerto si se conoce,
  resultado y modo). No se registran claves de API ni flags/argumentos del
  arranque.

### 3. Eliminación de modelos (`ropero <alias> remove`)

- La eliminación calcula un plan a partir de artefactos **derivables** del TOML
  y de las rutas resueltas del modelo (ficheros declarados/auxiliares de
  runtime y, en su caso, operación delegada FLM).
- Antes de ejecutar, se muestra el plan y se requiere **confirmación
  interactiva en un TTY**. Sin TTY, o si la confirmación no es afirmativa, no
  se elimina nada.
- Sólo se eliminan ficheros concretos atribuibles al modelo. No se borran
  directorios compartidos ni datos no declarados; el directorio concreto del
  modelo en `GGUF_DIR` sólo puede retirarse si queda vacío tras eliminar
  ficheros concretos.
- El directorio checkpoint `safetensors` identificado por `[model].path` se
  muestra como unidad del artefacto; tras confirmación se elimina esa ruta
  exacta bajo `SAFETENSORS_DIR`. Se rechazan escapes y symlinks intermedios; la
  raíz compartida nunca se borra.
- Para FLM delegado, la operación automática elimina todos los tags declarados
  que la instalación pudo descargar (modelo principal y auxiliares como el de
  decisión), ejecutando `flm remove <tag>` una vez por tag.

## Consecuencias

- El contrato operativo distingue entre estado inferido de mejor esfuerzo (FLM)
  y eliminación efectiva de artefactos locales concretos.
- Los logs de uso facilitan auditoría local básica sin persistir secretos.
- La eliminación prioriza seguridad frente a completitud automática: ante duda
  (safetensors, rutas no seguras, sin confirmación) se bloquea o cancela sin
  efectos destructivos.
