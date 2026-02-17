# Self-Hosted GitHub Actions Runner Setup

This repository CI is configured to run on `runs-on: self-hosted` for all jobs.

## Runner host requirements

Supported host OS for this workflow:
- Linux (recommended)
- macOS (supported)

Required tools on the runner host:
- `bash`, `curl`, `git`, `grep`, `sed`, `awk`, `tar`, `make`
- Rust stable toolchain (`rustup`, `cargo`, `rustc`)
- Rust components: `rustfmt`, `clippy`
- `cargo-audit`
- `gitleaks` (v8.30.0 or compatible)

This workflow does not require Docker.

## One-time host bootstrap

From the repo root:

```bash
bash scripts/setup_self_hosted_runner.sh
```

This installs/verifies Rust components, `cargo-audit`, and `gitleaks`, then runs CI preflight checks.

## Register the runner with this repository

1. Open the repository on GitHub.
2. Go to `Settings` -> `Actions` -> `Runners`.
3. Click `New self-hosted runner`.
4. Select your OS and architecture.
5. Run the exact commands shown by GitHub on your host. Example shape:

```bash
mkdir actions-runner && cd actions-runner
curl -o actions-runner.tar.gz -L https://github.com/actions/runner/releases/download/<version>/actions-runner-<os>-<arch>-<version>.tar.gz
tar xzf ./actions-runner.tar.gz
./config.sh --url https://github.com/sarveshkapre/embeddb --token <temporary-registration-token> --unattended --replace --name embeddb-self-hosted
```

6. Start the runner:

```bash
./run.sh
```

For persistent service mode, follow the service instructions printed by `config.sh` for your OS.

## Validate CI locally before pushing

Run the exact build job sequence locally:

```bash
bash scripts/ci_local_self_hosted.sh
```

or:

```bash
make ci-local-self-hosted
```

## Notes

- Keep the runner online while workflows execute.
- If multiple repositories share this runner host, isolate checkout/work directories per runner instance.
- Re-run `bash scripts/setup_self_hosted_runner.sh` after major toolchain upgrades.
