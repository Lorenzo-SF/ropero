#!/usr/bin/env bash
# =============================================================================
# test/test-nemotron.sh — batería de pruebas del modelo "nemotron"
# =============================================================================
# Wrapper mínimo: TODA la lógica vive en test/lib/test-common.sh (runner
# compartido), para que los siete scripts de modelo no vuelvan a divergir
# entre sí (lo que pasó en el refactor anterior).
#
# bash 3.2 (macOS, ADR-1): sin namerefs, sin arrays asociativos.
#
# USO:  ./test/test-nemotron.sh
#
# Qué deja en $HOME/test/nemotron/:
#   prueba1/  prueba2/  prueba3/  prueba_larga/  prueba_gorda/
#   recopilacion-completa.md   (con el contenido de cada batería)
#
# Cada batería deja en su directorio:
#   <bateria>.json · sesion-<bateria>.md · log-llama-sesion-<bateria>.txt
#   opencode-err-<bateria>.txt · opencode.json (provider hermético propio)
#
# AVISO: antes de cada batería el runner deja el entorno libre
# (`ropero stop all` y `pkill -9 -x` de llama-server/opencode/mcode/claude).
# No lo lances con trabajo abierto en esos agentes.
# =============================================================================
set -u

TEST_MODEL="nemotron"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR
# shellcheck source=lib/test-common.sh
. "$SCRIPT_DIR/lib/test-common.sh"

_test_run_all "$TEST_MODEL"
rc=$?
exit "$rc"
