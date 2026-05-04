# cslview
Open source viewer for map files generated via CSLMapView (CS:1).

## Build a release executable

Install the stable Rust toolchain with rustup, then build from the repository root:

```bash
cargo build --release --locked
```

The optimized executable is written to `target/release/`.

## Linux

```bash
cargo build --release --locked
./target/release/cslview
```

The release executable is `target/release/cslview`.

## Windows

```powershell
cargo build --release --locked
.\target\release\cslview.exe
```

The release executable is `target\release\cslview.exe`.

## macOS

```bash
cargo build --release --locked
./target/release/cslview
```

The release executable is `target/release/cslview`.
