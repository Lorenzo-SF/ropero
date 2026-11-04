#!/usr/bin/env bash
# =============================================================================
# router.sh — Routing de modelos usando d3:turbo (NPU)
# =============================================================================
# Uso: bin/router.sh "descripción de la tarea"
# Devuelve: el modelo seleccionado según MODELS.md
# =============================================================================

set -euo pipefail

INTENCION="${1:-}"
if [[ -z "$INTENCION" ]]; then
    echo "Uso: $0 \"descripción de la tarea\"" >&2
    exit 1
fi

# Verificar que d3:turbo está disponible
if ! command -v flm >/dev/null 2>&1; then
    echo "flm no está instalado" >&2
    exit 1
fi

# Endpoint de decisiones (debe estar corriendo)
D3_URL="${D3_URL:-http://127.0.0.1:9991/v1/systemone}"

# Intentar llamar al endpoint de decisiones
RESPONSE=$(curl -s -X POST "$D3_URL" \
    -H "Content-Type: application/json" \
    -d "{\"input\": \"$INTENCION\"}" 2>/dev/null || echo "")

if [[ -z "$RESPONSE" ]]; then
    # Fallback: usar lfm2.5 para clasificación básica
    echo "minimax/m3.1-flash"
    exit 0
fi

# Extraer el modelo seleccionado (ajustar según el formato de respuesta)
echo "$RESPONSE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('model', 'minimax/m3.1-flash'))" 2>/dev/null || echo "minimax/m3.1-flash"
