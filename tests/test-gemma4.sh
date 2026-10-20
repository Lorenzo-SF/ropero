#!/usr/bin/env bash
# Offline contract: three downloads, two fixed launchers. No weights or servers.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export QA_REPO QA_BASH
QA_REPO="$(cd "$HERE/.." && pwd)"
QA_BASH="$(command -v bash)"
python3 - "$@" <<'PY'
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
        MODEL_NAMES+=(qa_alias); MODEL_ALIAS_TARGET[qa_alias]=gemma4
        MODEL_GGUF[qa_alias]="${MODEL_GGUF[gemma4]}"
        _ropero_cat_index qa_alias; exit $? ;;
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


class GemmaContract(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory(prefix='ropero-gemma4-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(tmp.cleanup)
        self.work = Path(tmp.name)
        shutil.copytree(REPO / 'ropero.d', self.work / 'ropero.d')
        source = (REPO / 'ropero').read_text()
        needle = '\nmain "$@"\n'
        self.assertEqual(source.count(needle), 1, 'instrument only the main call')
        (self.work / 'ropero').write_text(source.replace(needle, '\n' + HOOK + needle))
        for directory in ('home', 'models with spaces', 'bin', 'tmp', 'logs'):
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
        for tool in ('curl wget git cmake ninja make brew sudo pip pip3 uv huggingface-cli '
                     'flm lsof ss nc ssh python python3 apt apt-get dnf pacman paru pkg').split():
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
        result = subprocess.run([BASH, str(self.work / 'ropero'), *args], env=self.env,
                                cwd=self.work, text=True, capture_output=True, timeout=20)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def transfers(self) -> list[str]:
        path = self.work / 'transfers'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def trio(self, root: Path | None = None) -> None:
        for name in (MAIN, PROJ, MTP):
            self.assertTrue(((root or self.models) / name).is_file(), name)
            self.assertGreater(((root or self.models) / name).stat().st_size, 0)

    def test_catalog_and_two_scripts_share_single_primary(self) -> None:
        catalog = self.run_ropero('qa-catalog').stdout.splitlines()
        self.assertEqual(catalog.count(MAIN), 1)
        self.assertNotIn(PROJ, catalog)
        self.assertNotIn(MTP, catalog)
        index = self.run_ropero('qa-index', MAIN).stdout
        for selector in ('gemma4', 'gemma4-ctx'):
            self.assertEqual(self.run_ropero('qa-index', selector).stdout, index)
        self.assertEqual(self.run_ropero('qa-alias').stdout, index)

    def test_named_download_and_projector_isolation(self) -> None:
        del self.env['AUTO_YES'], self.env['CI']
        generic = self.models / 'mmproj-BF16.gguf'
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
        (self.models / MAIN).write_bytes(b'existing primary')
        self.run_ropero('qa-fetch', 'gemma4')
        self.trio()
        self.assertCountEqual(self.transfers(), REMOTES[1:])
        self.assertEqual((self.models / MAIN).read_bytes(), b'existing primary')

    def test_idempotence_and_force(self) -> None:
        self.run_ropero('qa-fetch', 'gemma4')
        self.run_ropero('qa-fetch', 'gemma4-ctx')
        self.assertCountEqual(self.transfers(), REMOTES)
        self.env['ROPERO_FORCE'] = '1'
        self.run_ropero('qa-fetch', 'gemma4')
        self.assertCountEqual(self.transfers(), REMOTES * 2)
        self.trio()

    def test_primary_failure_does_not_download_auxiliaries(self) -> None:
        self.env['QA_FAIL'] = MAIN
        self.assertNotEqual(self.run_ropero('qa-fetch', 'gemma4', ok=False).returncode, 0)
        self.assertEqual(self.transfers(), [MAIN])
        self.assertFalse((self.models / PROJ).exists())
        self.assertFalse((self.models / MTP).exists())

    def test_declined_primary_does_not_download_auxiliaries(self) -> None:
        del self.env['AUTO_YES'], self.env['CI']
        self.env['QA_DECLINE'] = '1'
        self.run_ropero('qa-fetch', 'gemma4', ok=False)
        self.assertEqual(self.transfers(), [])
        self.assertEqual(list(self.models.iterdir()), [])

    def test_projector_failure_propagates(self) -> None:
        self.env['QA_FAIL'] = REMOTES[1]
        self.assertNotEqual(self.run_ropero('qa-fetch', 'gemma4', ok=False).returncode, 0)

    def test_mtp_failure_propagates(self) -> None:
        self.env['QA_FAIL'] = MTP
        self.assertNotEqual(self.run_ropero('qa-fetch', 'gemma4', ok=False).returncode, 0)

    def test_default_directory_and_curl(self) -> None:
        del self.env['GGUF_DIR']
        # El camino sin 'hf' se fuerza con ROPERO_DOWNLOADER=curl en vez de
        # borrar el binario: si la máquina donde corre el test tiene 'hf' de
        # sistema (en /usr/bin, como en la mayoría), quitar el stub de verdad
        # lo dejaba encontrar el de sistema y se ponía a descargar 14 GB de
        # verdad. Con el override, el test dice lo que quiere decir.
        self.env['ROPERO_DOWNLOADER'] = 'curl'
        self.executable('curl', '#!' + sys.executable + '\n' + TRANSPORT)
        self.run_ropero('qa-fetch', 'gemma4')
        self.trio(self.work / 'home' / 'models' / 'gguf')
        self.assertCountEqual(self.transfers(), REMOTES)

    def test_selector_deduplicates_both_launchers(self) -> None:
        self.run_ropero('models', 'gemma4,gemma4-ctx')
        self.assertCountEqual(self.transfers(), REMOTES)
        self.trio()

    def test_numeric_selector(self) -> None:
        number = self.run_ropero('qa-menu', 'gemma4_ctx').stdout.strip()
        self.assertTrue(number.isdigit(), number)
        self.run_ropero('models', number)
        self.assertCountEqual(self.transfers(), REMOTES)
        self.trio()

    def test_bulk_selector(self) -> None:
        self.run_ropero('models', '99')
        self.trio()
        for remote in REMOTES:
            self.assertEqual(self.transfers().count(remote), 1, remote)

    def test_fixed_launcher_argv_and_standalone_scripts(self) -> None:
        self.run_ropero('qa-fetch', 'gemma4')
        for model, ctx in (('gemma4', '49152'), ('gemma4-ctx', '262144')):
            with self.subTest(model=model):
                # Source each script in isolation: no shared Gemma helper required.
                script = self.work / 'ropero.d' / (model + '.sh')
                standalone = subprocess.run(
                    [BASH, '-c', 'source "$1"; "get_model_args_${2//-/_}"', '_', str(script), model],
                    env=self.env, text=True, capture_output=True, timeout=10)
                self.assertEqual(standalone.returncode, 0, standalone.stderr)
                self.run_ropero(model)
                argv = (self.work / 'argv').read_bytes().decode().rstrip('\0').split('\0')
                pairs = {'--model': str(self.models / MAIN), '--ctx-size': ctx,
                         '--n-gpu-layers': 'all', '--spec-draft-ngl': 'all',
                         '--model-draft': str(self.models / MTP), '--spec-type': 'draft-mtp',
                         '--spec-draft-n-max': '2', '--mmproj': str(self.models / PROJ),
                         '--parallel': '1', '--fit': 'off', '--cache-type-k': 'q4_0',
                         '--cache-type-v': 'q4_0', '--batch-size': '512', '--ubatch-size': '512',
                         '--top-p': '0.95', '--top-k': '64'}
                for flag, value in pairs.items():
                    self.assertEqual(argv.count(flag), 1, flag)
                    self.assertEqual(argv[argv.index(flag) + 1], value, flag)
                self.assertIn('--jinja', argv)
                self.assertEqual(argv.count('--temp'), 1)
                self.assertEqual(float(argv[argv.index('--temp') + 1]), 1.0)
                if model == 'gemma4':
                    self.assertNotIn('--n-cpu-moe', argv)
                    self.assertIn('--mmproj-offload', argv)
                    self.assertNotIn('--no-mmproj-offload', argv)
                else:
                    self.assertEqual(argv.count('--n-cpu-moe'), 1)
                    self.assertEqual(argv[argv.index('--n-cpu-moe') + 1], '5')
                    self.assertIn('--no-mmproj-offload', argv)
                    self.assertNotIn('--mmproj-offload', argv)


unittest.main(argv=[sys.argv[0], *sys.argv[1:]], verbosity=2)
PY
