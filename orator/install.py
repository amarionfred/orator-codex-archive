"""Install Orator. --activate also installs the Codex binary."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--activate", action="store_true")
args = parser.parse_args()
data = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "orator"
data.mkdir(parents=True, exist_ok=True)
python = data / "venv/bin/python"
if not python.exists():
    subprocess.run(["uv", "venv", "--python", "3.12", str(data / "venv")], check=True)
subprocess.run(
    [
        "uv",
        "pip",
        "install",
        "--python",
        str(python),
        "-r",
        str(root / "orator/requirements.txt"),
    ],
    check=True,
)
backend = data / "backend.py"
backend.write_text(f"#!{python}\n" + (root / "orator/backend.py").read_text())
backend.chmod(0o755)
(data / "bin").mkdir(exist_ok=True)
worker = data / "bin/orator-backend"
worker.unlink(missing_ok=True)
worker.symlink_to(backend)
cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
model = cache / "orator/models/base"
if not (model / "model.bin").exists():
    previous = cache / "agent-voice/models/base"
    model.parent.mkdir(parents=True, exist_ok=True)
    if (previous / "model.bin").exists():
        shutil.move(str(previous), model)
    else:
        subprocess.run(
            [
                str(python),
                "-c",
                "from huggingface_hub import snapshot_download; "
                f"snapshot_download('Systran/faster-whisper-base', local_dir={str(model)!r})",
            ],
            check=True,
        )
config = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "orator"
config.mkdir(parents=True, exist_ok=True)
if not (config / "config.toml").exists():
    (config / "config.toml").write_text(
        'model = "base"\nlanguage = "auto"\nsource = "default"\nmax_seconds = 180\ncpu_threads = 4\n'
    )
if args.activate:
    binary = root / "codex-rs/target/dev-small/codex"
    built_version = subprocess.check_output(
        [str(binary), "--version"], text=True
    ).strip()
    installed_version = subprocess.check_output(
        ["/usr/bin/codex", "--version"], text=True
    ).strip()
    if built_version != installed_version:
        raise SystemExit(
            "Build and system Codex versions differ; install the matching code-mode helper first."
        )
    local_bin = Path.home() / ".local/bin"
    launcher = local_bin / "codex"
    if (
        launcher.exists() or launcher.is_symlink()
    ) and launcher.resolve() != data / "codex":
        raise SystemExit(f"Refusing to replace unrelated {launcher}")
    shutil.copy2(binary, data / "codex.new")
    (data / "codex.new").replace(data / "codex")
    # Keep the code-mode runtime from the matching installed Codex release.
    helpers = Path(
        "/usr/lib/node_modules/@openai/codex/node_modules/@openai/codex-linux-x64/vendor/x86_64-unknown-linux-musl/bin"
    )
    if (helpers / "codex-code-mode-host").is_file():
        shutil.copy2(helpers / "codex-code-mode-host", data / "codex-code-mode-host")
    launcher.unlink(missing_ok=True)
    launcher.symlink_to(data / "codex")
    for name in ("codex-voice", "agent-voice"):
        old = local_bin / name
        if old.is_symlink() and "Projects/agent-voice/" in str(old.resolve()):
            old.unlink()
print(
    "Orator installed."
    + (" Open codex and use /record." if args.activate else " Speech worker installed.")
)
