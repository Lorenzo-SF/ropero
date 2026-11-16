# ADR-011 · Compilar motores declarados por manifiesto

- **Estado:** aceptado para Ropero v2
- **Contexto:** `ropero install engines` solo intentaba resolver un ejecutable y
  crear un symlink; al faltar el binario decía al usuario que lo instalara por
  separado. Eso no satisface la petición de compilar motores seleccionados.
- **Decisión:** un manifiesto puede declarar `[install]` con `source`,
  `source_dir`, `target`, `binary` y `cmake_args`. Runtime ejecuta `git clone` y
  `cmake` como procesos separados (sin shell), compila y después enlaza el
  ejecutable. El build reutiliza un checkout con `CMakeLists.txt`, pero nunca
  borra un directorio fuente no vacío inválido. Sin receta, sigue siendo
  necesario instalar el ejecutable por otro medio.
- **Recetas iniciales:** `llama-server` compila upstream `ggml-org/llama.cpp`.
  `turbo-llama` compila `lorenzo-sf/turbo-llama`, el fork público del dueño.
  Ambos comparten capacidades, plataformas y argumentos de compilación en
  Linux y macOS. Las recetas requieren Git, CMake y Ninja disponibles en
  `PATH`; dependencias CUDA/Metal específicas quedan a cargo del entorno CMake.
- **Consecuencias:** la selección `install engines` puede compilar código y
  acceder a red. El usuario ya autoriza esa operación al seleccionar el motor;
  errores de dependencias, clone, configure o build se propagan sin crear un
  enlace falso. No se sobrescribe el destino de un fichero real.
- **Actualización:** el repositorio `lorenzo-sf/turbo-llama` ya está disponible;
  el manifiesto usa esa URL y no mantiene una receta fallback.
