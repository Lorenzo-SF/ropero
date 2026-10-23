#!/usr/bin/env bash
# Offline contract: bundles de modelo (principal + ficheros secundarios).
# Sin pesos ni servidores: todo el transporte es un stub.
#
# Cubre los DOS modelos del catálogo que se descargan como BUNDLE:
#   · Gemma 4 26B-A4B  → GGUF + MTP + mmproj
#   · Qwen3.8 27B      → GGUF + MTP + mmproj   (el MTP vive en MTP/ del repo)
#
# El segundo caso está aquí por un motivo concreto: el MTP de Qwen3.8 está en
# un SUBDIRECTORIO del repo ("MTP/mtp-Qwen3.8-27B-Q4_0.gguf"), y ese camino
# es el que rompía el rename al mover el fichero a la raíz de GGUF_DIR.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export QA_REPO QA_BASH
QA_REPO="$(cd "$HERE/.." && pwd)"
QA_BASH="$(command -v bash)"

# Un python3 que FUNCIONE. El shim de asdf puede estar en el PATH y estar roto
# ("No version is set for command python3"), así que se prueban los candidatos
# en vez de coger el primero que aparece.
QA_PYTHON=""
for cand in python3 python /usr/bin/python3 /usr/local/bin/python3 /bin/python3; do
    if "$cand" -c 'import sys' >/dev/null 2>&1; then QA_PYTHON="$cand"; break; fi
done
if [[ -z "$QA_PYTHON" ]]; then
    for cand in "$HOME"/.asdf/installs/python/*/bin/python3; do
        if [[ -x "$cand" ]] && "$cand" -c 'import sys' >/dev/null 2>&1; then
            QA_PYTHON="$cand"; break
        fi
    done
fi
if [[ -z "$QA_PYTHON" ]]; then
    echo "no encuentro un python3 utilizable" >&2
    exit 1
fi

"$QA_PYTHON" - "$@" <<'PY'
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = Path(os.environ['QA_REPO'])
BASH = os.environ['QA_BASH']
MAIN = 'gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf'
PROJ = 'gemma-4-26B-A4B-it-mmproj-BF16.gguf'
MTP = 'mtp-gemma-4-26B-A4B-it.gguf'
REMOTES = [MAIN, 'mmproj-BF16.gguf', MTP]

# Qwen3.8: el MTP está en un subdirectorio del repo ("speculative/…") y el
# mmproj tiene nombre propio → de ahí el renombrado.
QMAIN = 'Qwen3.8-27B-EfficientThink-VAL-APEX-I-MiniPlus-V3.gguf'
QMTP = 'dflash2-qwen38-27b-Q4_K_M.gguf'
QPROJ = 'mmproj-Qwen3.8-27B-Q4_K_M.gguf'
QREMOTES = [QMAIN, 'speculative/dflash2-qwen38-27b-Q4_K_M.gguf', 'mmproj-Qwen3.8-27B-Q4_K_M.gguf']

HOOK = r'''
ropero_env_setup() { :; }
ropero_detect_os() { printf unknown; }
ropero_ask() {
    [[ "${AUTO_YES:-0}" != 1 ]] || return 0
    printf 'prompt\n' >> "$QA_PROMPTS"
    [[ "${QA_DECLINE:-0}" != 1 ]]
}
port_listening() { return 1; }
colorize() { cat; }
alaja() { :; }
_kill_port() { printf 'unexpected kill\n' >> "$QA_BLOCKED"; return 125; }
_stop_port() { printf 'unexpected stop\n' >> "$QA_BLOCKED"; return 125; }
case "${1:-}" in
    qa-index) _ropero_cat_index "$2"; exit $? ;;
    qa-catalog) _ropero_cat_build; printf '%s\n' "${_ROPERO_CAT_KEY[@]}"; exit 0 ;;
    qa-fetch) idx=$(_ropero_cat_index "$2") || exit; _ropero_cat_download "$idx"; exit $? ;;
    qa-menu)
        declare -gA ROPERO_MENU_MODELS=()
        _ropero_models_menu >/dev/null
        for n in "${!ROPERO_MENU_MODELS[@]}"; do
            [[ "${ROPERO_MENU_MODELS[$n]}" != "$2" ]] || printf '%s\n' "$n"
        done
        exit 0 ;;
    qa-alias)
        MODEL_NAMES+=(qa_alias); MODEL_ALIAS_TARGET[qa_alias]=gemma
        MODEL_GGUF[qa_alias]="${MODEL_GGUF[gemma]}"
        _ropero_cat_index qa_alias; exit $? ;;
    qa-aux-subdir)
        # Entrada sintética con un aux cuyo fichero vive en un SUBDIRECTORIO
        # del repo ("MTP/x.gguf"). El MTP de Qwen3.8 es exactamente ese caso,
        # y es el que rompía el rename: al moverlo a la raíz de GGUF_DIR hay
        # que sacarlo de MTP/ primero.
        #
        # El formato del aux es "repo|ruta/en/repo|nombre final|tamaño|etiqueta".
        _ropero_cat_build
        _cat_gguf "sintetico.gguf" "unsloth/ejemplo" "sintetico.gguf" "Sintetico" "1 MB" "$GGUF_DIR/gemma"
        _cat_aux "sintetico.gguf" \
            "unsloth/ejemplo|MTP/mtp-ejemplo-Q4_0.gguf|mtp-ejemplo-Q4_0.gguf|1.37 GB|MTP de ejemplo"
        idx=$(_ropero_cat_index "sintetico.gguf") || exit
        _ropero_cat_download "$idx"; exit $? ;;
esac
'''
TRANSPORT = r'''
import json, os, pathlib, sys
args = sys.argv[1:]
if pathlib.Path(sys.argv[0]).name == 'hf':
    remote = args[2]
    dest = pathlib.Path(args[args.index('--local-dir') + 1]) / remote
else:
    remote = next(a for a in args if a.startswith('https://')).split('/resolve/main/')[1]
    dest = pathlib.Path(args[args.index('-o') + 1])
with open(os.environ['QA_TRANSFERS'], 'a') as log:
    log.write(json.dumps(remote) + '\n')
if remote == os.environ.get('QA_FAIL'):
    sys.exit(47)
dest.parent.mkdir(parents=True, exist_ok=True)
dest.write_bytes(b'GGUF tiny offline test\n')
'''


class BundleContract(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory(prefix='ropero-bundle-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(tmp.cleanup)
        self.work = Path(tmp.name)
        # Estructura nueva: el entrypoint y los módulos van en bin/, los
        # modelos en models/ y el servidor de endpoints en endpoints/.
        shutil.copytree(REPO / 'bin', self.work / 'bin')
        shutil.copytree(REPO / 'models', self.work / 'models')
        shutil.copytree(REPO / 'endpoints', self.work / 'endpoints')
        source = (self.work / 'bin' / 'ropero').read_text()
        needle = '\nmain "$@"\n'
        self.assertEqual(source.count(needle), 1, 'instrument only the main call')
        (self.work / 'bin' / 'ropero').write_text(source.replace(needle, '\n' + HOOK + needle))
        # 'bin' ya existe: lo ha creado el copytree del árbol de ropero.
        for directory in ('home', 'models with spaces', 'tmp', 'logs'):
            (self.work / directory).mkdir()
        self.models = self.work / 'models with spaces'
        self.env = {'PATH': str(self.work / 'bin') + ':/usr/bin:/bin',
                    'HOME': str(self.work / 'home'), 'GGUF_DIR': str(self.models),
                    'TMPDIR': str(self.work / 'tmp'), 'LOG_DIR': str(self.work / 'logs'),
                    'LLAMA_DIR': str(self.work / 'llama'), 'BIN_DIR': str(self.work / 'bin'),
                    'LLAMA_API_KEY': 'offline-test-only', 'AUTO_YES': '1', 'CI': '1',
                    'QA_TRANSFERS': str(self.work / 'transfers'),
                    'QA_PROMPTS': str(self.work / 'prompts'),
                    'QA_BLOCKED': str(self.work / 'blocked'),
                    'LLAMA_BIN': str(self.work / 'bin' / 'llama-server')}
        block = '#!/bin/bash\nprintf "%s\\n" "$0" >> "$QA_BLOCKED"\nexit 125\n'
        # python3 NO se bloquea: los modelos se configuran en TOML y su
        # lector es un script de Python. Lo que sigue bloqueado es todo lo
        # que puede RED (curl, wget, git, hf, pip) y lo que puede tocar el
        # sistema. El lector de TOML no hace red.
        for tool in ('curl wget git cmake ninja make brew sudo pip pip3 uv huggingface-cli '
                     'flm lsof ss nc ssh apt apt-get dnf pacman paru pkg').split():
            self.executable(tool, block)
        self.executable('hf', '#!' + sys.executable + '\n' + TRANSPORT)
        self.executable('llama-server', '#!/bin/bash\nprintf "%s\\0" "$@" > "$QA_ARGV"\n')
        self.env['QA_ARGV'] = str(self.work / 'argv')

    def tearDown(self) -> None:
        self.assertFalse((self.work / 'blocked').exists(), 'unexpected external tool reached')

    def executable(self, name: str, text: str) -> None:
        path = self.work / 'bin' / name
        path.write_text(text)
        path.chmod(0o755)

    def run_ropero(self, *args: str, ok: bool = True) -> subprocess.CompletedProcess:
        result = subprocess.run([BASH, str(self.work / 'bin' / 'ropero'), *args], env=self.env,
                                cwd=self.work, text=True, capture_output=True, timeout=20)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def transfers(self) -> list[str]:
        path = self.work / 'transfers'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def argv(self) -> list[str]:
        return (self.work / 'argv').read_bytes().decode().rstrip('\0').split('\0')

    def trio(self, root: Path | None = None) -> None:
        base = (root or self.models) / 'gemma'
        for name in (MAIN, PROJ, MTP):
            self.assertTrue((base / name).is_file(), name)
            self.assertGreater((base / name).stat().st_size, 0)

    # ── Catálogo ────────────────────────────────────────────────────────────
    def test_catalog_has_single_primary(self) -> None:
        catalog = self.run_ropero('qa-catalog').stdout.splitlines()
        self.assertEqual(catalog.count(MAIN), 1)
        self.assertNotIn(PROJ, catalog)
        self.assertNotIn(MTP, catalog)
        index = self.run_ropero('qa-index', MAIN).stdout
        self.assertEqual(self.run_ropero('qa-index', 'gemma').stdout, index)
        self.assertEqual(self.run_ropero('qa-alias').stdout, index)

    # ── Gemma 4 ────────────────────────────────────────────────────────────
    def test_named_download_and_projector_isolation(self) -> None:
        del self.env['AUTO_YES'], self.env['CI']
        generic = self.models / 'gemma' / 'mmproj-BF16.gguf'
        generic.parent.mkdir(parents=True, exist_ok=True)
        generic.write_bytes(b'unrelated projector')
        before = generic.stat()
        self.run_ropero('qa-fetch', MAIN)
        self.trio()
        self.assertCountEqual(self.transfers(), REMOTES)
        self.assertEqual((self.work / 'prompts').read_text().splitlines(), ['prompt'])
        self.assertEqual(generic.read_bytes(), b'unrelated projector')
        self.assertEqual(generic.stat().st_ino, before.st_ino)
        self.assertEqual(generic.stat().st_mtime_ns, before.st_mtime_ns)

    def test_existing_primary_repairs_auxiliaries(self) -> None:
        (self.models / 'gemma').mkdir(parents=True, exist_ok=True)
        (self.models / 'gemma' / MAIN).write_bytes(b'existing primary')
        self.run_ropero('qa-fetch', 'gemma')
        self.trio()
        self.assertCountEqual(self.transfers(), REMOTES[1:])
        self.assertEqual((self.models / 'gemma' / MAIN).read_bytes(), b'existing primary')

    def test_idempotence_and_force(self) -> None:
        self.run_ropero('qa-fetch', 'gemma')
        self.run_ropero('qa-fetch', 'gemma')
        self.assertCountEqual(self.transfers(), REMOTES)
        self.env['ROPERO_FORCE'] = '1'
        self.run_ropero('qa-fetch', 'gemma')
        self.assertCountEqual(self.transfers(), REMOTES * 2)
        self.trio()

    def test_primary_failure_does_not_download_auxiliaries(self) -> None:
        self.env['QA_FAIL'] = MAIN
        self.assertNotEqual(self.run_ropero('qa-fetch', 'gemma', ok=False).returncode, 0)
        self.assertEqual(self.transfers(), [MAIN])
        self.assertFalse((self.models / 'gemma' / PROJ).exists())
        self.assertFalse((self.models / 'gemma' / MTP).exists())

    def test_declined_primary_does_not_download_auxiliaries(self) -> None:
        del self.env['AUTO_YES'], self.env['CI']
        self.env['QA_DECLINE'] = '1'
        self.run_ropero('qa-fetch', 'gemma', ok=False)
        self.assertEqual(self.transfers(), [])
        self.assertEqual(list(self.models.iterdir()), [])

    def test_projector_failure_propagates(self) -> None:
        self.env['QA_FAIL'] = REMOTES[1]
        self.assertNotEqual(self.run_ropero('qa-fetch', 'gemma', ok=False).returncode, 0)

    def test_mtp_failure_propagates(self) -> None:
        self.env['QA_FAIL'] = MTP
        self.assertNotEqual(self.run_ropero('qa-fetch', 'gemma', ok=False).returncode, 0)

    def test_default_directory_and_curl(self) -> None:
        del self.env['GGUF_DIR']
        # El camino sin 'hf' se fuerza con ROPERO_DOWNLOADER=curl en vez de
        # borrar el binario: si la máquina donde corre el test tiene 'hf' de
        # sistema (en /usr/bin, como en la mayoría), quitar el stub de verdad
        # lo dejaba encontrar el de sistema y se ponía a descargar 14 GB de
        # verdad. Con el override, el test dice lo que quiere decir.
        self.env['ROPERO_DOWNLOADER'] = 'curl'
        self.executable('curl', '#!' + sys.executable + '\n' + TRANSPORT)
        self.run_ropero('qa-fetch', 'gemma')
        self.trio(self.work / 'home' / 'models' / 'gguf')
        self.assertCountEqual(self.transfers(), REMOTES)

    def test_selector_downloads_bundle(self) -> None:
        self.run_ropero('models', 'gemma')
        self.assertCountEqual(self.transfers(), REMOTES)
        self.trio()

    def test_numeric_selector(self) -> None:
        number = self.run_ropero('qa-menu', 'gemma').stdout.strip()
        self.assertTrue(number.isdigit(), number)
        self.run_ropero('models', number)
        self.assertCountEqual(self.transfers(), REMOTES)
        self.trio()

    def test_bulk_selector(self) -> None:
        self.run_ropero('models', '99')
        self.trio()
        for remote in REMOTES:
            self.assertEqual(self.transfers().count(remote), 1, remote)

    # ── Launcher: la visión es opt-in y el MoE se puede ajustar ────────────
    def test_launcher_vision_is_opt_in_and_moe_adjustable(self) -> None:
        self.run_ropero('qa-fetch', 'gemma')

        # 1) Sin --vision: el modelo arranca SIN torre de visión. El
        #    --n-cpu-moe 5 viene por defecto SIEMPRE (con visión o sin ella):
        #    en esta máquina (16 GB de VRAM) sin ese hueco hay OOM.
        self.run_ropero('gemma')
        argv = self.argv()
        self.assertNotIn('--mmproj', argv)
        self.assertEqual(argv[argv.index('--ctx-size') + 1], '131072')
        self.assertEqual(argv[argv.index('--n-gpu-layers') + 1], '99')
        self.assertEqual(argv[argv.index('--n-cpu-moe') + 1], '5')
        self.assertEqual(argv[argv.index('--cache-type-k') + 1], 'q8_0')
        self.assertEqual(argv[argv.index('--cache-type-v') + 1], 'q8_0')
        self.assertEqual(argv[argv.index('--spec-type') + 1], 'draft-mtp')
        self.assertEqual(argv[argv.index('--model-draft') + 1], str(self.models / 'gemma' / MTP))
        self.assertIn('--jinja', argv)
        self.assertEqual(float(argv[argv.index('--temp') + 1]), 1.0)
        self.assertEqual(argv[argv.index('--top-p') + 1], '0.95')
        self.assertEqual(argv[argv.index('--top-k') + 1], '64')
        # El thinking va en las kwargs del chat template.
        kwargs = argv[argv.index('--chat-template-kwargs') + 1]
        self.assertIn('"enable_thinking":true', kwargs)

        # 2) Con --vision: aparece el mmproj y hacen falta capas de expertos
        #    en la CPU para hacerle hueco (default 5).
        self.run_ropero('gemma', '--vision')
        argv = self.argv()
        self.assertEqual(argv.count('--mmproj'), 1)
        self.assertEqual(argv[argv.index('--mmproj') + 1], str(self.models / 'gemma' / PROJ))
        self.assertEqual(argv[argv.index('--n-cpu-moe') + 1], '5')

        # 3) --n-cpu-moe ajusta los expertos MoE en la CPU, con y sin visión.
        self.run_ropero('gemma', '--vision', '--n-cpu-moe', '8')
        argv = self.argv()
        self.assertEqual(argv[argv.index('--n-cpu-moe') + 1], '8')
        self.assertEqual(argv.count('--mmproj'), 1)
        self.run_ropero('gemma', '--n-cpu-moe', '2')
        argv = self.argv()
        self.assertEqual(argv[argv.index('--n-cpu-moe') + 1], '2')
        self.assertNotIn('--mmproj', argv)

        # 4) Un flag que el modelo NO declara es un error de uso.
        result = self.run_ropero('gemma', '--vision', '--inventado', ok=False)
        self.assertNotEqual(result.returncode, 0)

    def test_launcher_levels_map_to_thinking(self) -> None:
        self.run_ropero('qa-fetch', 'gemma')
        for level, thinking, preserve in (('low', 'false', 'false'),
                                          ('medium', 'true', 'false'),
                                          ('high', 'true', 'true')):
            with self.subTest(level=level):
                self.run_ropero('gemma', '--' + level)
                argv = self.argv()
                kwargs = argv[argv.index('--chat-template-kwargs') + 1]
                self.assertIn(f'"enable_thinking":{thinking}', kwargs)
                self.assertIn(f'"preserve_thinking":{preserve}', kwargs)

    # ── Alias con nivel por defecto ────────────────────────────────────────
    def test_alias_carries_its_default_level(self) -> None:
        # `ropero verifier` es `ropero gptoss --high`: el alias público lleva
        # su nivel por defecto. Y el flag explícito gana sobre el del alias.
        (self.models / 'gptoss').mkdir(parents=True, exist_ok=True)
        (self.models / 'gptoss' / 'openai_gpt-oss-20b-MXFP4.gguf').write_bytes(b'fake gptoss')
        self.run_ropero('verifier')
        argv = self.argv()
        kwargs = argv[argv.index('--chat-template-kwargs') + 1]
        self.assertIn('"reasoning_effort":"high"', kwargs)
        # El flag explícito manda sobre el default del alias.
        self.run_ropero('verifier', '--low')
        argv = self.argv()
        kwargs = argv[argv.index('--chat-template-kwargs') + 1]
        self.assertIn('"reasoning_effort":"low"', kwargs)
        # Y `designer` es medium, no high.
        self.run_ropero('designer')
        argv = self.argv()
        kwargs = argv[argv.index('--chat-template-kwargs') + 1]
        self.assertIn('"reasoning_effort":"medium"', kwargs)

    # ── Aux en un SUBDIRECTORIO del repo (el caso del MTP de Qwen3.8) ──────
    def test_aux_in_repo_subdirectory_lands_in_root(self) -> None:
        # El MTP de Qwen3.8 está en "MTP/mtp-Qwen3.8-27B-Q4_0.gguf" dentro
        # del repo, pero el .sh del modelo lo busca en la RAÍZ de GGUF_DIR.
        # Si el rename no sacase el fichero de MTP/ primero, se quedaría
        # anidado y el modelo no encontraría el drafter.
        self.run_ropero('qa-aux-subdir')
        self.assertIn('MTP/mtp-ejemplo-Q4_0.gguf', self.transfers())
        self.assertTrue((self.models / 'gemma' / 'mtp-ejemplo-Q4_0.gguf').is_file())
        self.assertFalse((self.models / 'MTP').exists(),
                         'el MTP no debe quedar anidado en MTP/ dentro de GGUF_DIR')

    # ── Qwen3.8: el bundle completo cuando su entrada está activa ──────────
    def test_qwen38_bundle_with_subdirectory_mtp(self) -> None:
        catalog = self.run_ropero('qa-catalog').stdout.splitlines()
        if QMAIN not in catalog:
            self.skipTest('la entrada de Qwen3.8 está desactivada en el catálogo')
        # Un mmproj genérico ya presente NO se pisa: el de Qwen3.8 va a su
        # propio nombre (qwen3.8-mmproj-BF16.gguf).
        generic = self.models / 'gemma' / 'mmproj-BF16.gguf'
        generic.parent.mkdir(parents=True, exist_ok=True)
        generic.write_bytes(b'unrelated projector')
        before = generic.stat()
        self.run_ropero('qa-fetch', QMAIN)
        self.assertCountEqual(self.transfers(), QREMOTES)
        self.assertTrue((self.models / 'qwen' / QMAIN).is_file())
        self.assertTrue((self.models / 'qwen' / QMTP).is_file())
        self.assertTrue((self.models / 'qwen' / QPROJ).is_file())
        # El MTP venía de MTP/ en el repo y ha acabado en la raíz.
        self.assertFalse((self.models / 'qwen' / 'MTP').exists())
        self.assertEqual(generic.read_bytes(), b'unrelated projector')
        self.assertEqual(generic.stat().st_ino, before.st_ino)


unittest.main(argv=[sys.argv[0], *sys.argv[1:]], verbosity=2)
PY
