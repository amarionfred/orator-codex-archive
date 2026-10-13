# Orator

Local speech to text in Codex CLI. Type `/record`, speak, and press Enter.
The transcript goes into your draft. Edit it, then send it when you're ready.
Esc cancels recording or transcription.

Orator adds a native recording panel to Codex CLI 0.154.0. Transcription runs
on your CPU with Whisper. No speech API key is needed, and recordings are deleted
after transcription. Linux only.

## Install

Requires Rust, Python 3, `uv`, `parecord` (PulseAudio utilities), and a running
PulseAudio or PipeWire server. The current installer expects Codex CLI 0.154.0
at `/usr/bin/codex`, installed through the system-wide npm directory
`/usr/lib/node_modules`. It reuses that installation's code-mode runtime.

```sh
git clone https://github.com/amarionfred/orator.git
cd orator/codex-rs
cargo +stable build --locked --profile dev-small -p codex-cli --bin codex -j 2
cd ..
python3 orator/install.py --activate
```

The first install downloads the speech model. Later recordings work offline.
Keep `~/.local/bin` before `/usr/bin` in your `PATH`, then open a new `codex` session.

## Settings

Edit `~/.config/orator/config.toml`:

```toml
model = "base"
language = "auto"
source = "default"
max_seconds = 180
cpu_threads = 4
```

Set `language = "en"` for English. Use `pactl list short sources` to find a
microphone name for `source`; avoid sources ending in `.monitor`, which capture
speaker output. If audio clips, lower the microphone volume in your sound settings.
Capture uses only the microphone's first channel, with channel mixing disabled.

The model is stored in `~/.cache/orator/models/base`. The executable and speech
worker are installed under `~/.local/share/orator`.

## Development

The recording panel lives in `codex-rs/tui/src/bottom_pane/orator.rs`.
The speech worker and installer are in `orator/`.

With `just` and `cargo-nextest` installed:

```sh
RUSTUP_TOOLCHAIN=stable env -u NO_COLOR just test -p codex-tui --cargo-profile dev-small
python3 orator/qa_backend.py --audio /path/to/speech.wav --expect "words in the recording"
```

The audio check uses a virtual input. It does not record your microphone.
It requires FFmpeg with PulseAudio support. To check that the second channel
stays out of the recording:

```sh
~/.local/share/orator/venv/bin/python orator/test_capture.py
```

## Updates and removal

This is a source build based on [Codex CLI 0.154.0](https://github.com/openai/codex/tree/rust-v0.154.0).
Updating the system Codex package does not update Orator.
Remove the `~/.local/bin/codex` symlink to return to the system installation.

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for upstream attribution.
