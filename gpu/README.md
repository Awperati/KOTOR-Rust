# kotor-gpu

GPU renderer for `kotor-render` scenes (wgpu), offscreen first. Standalone package with its own lockfile so the main
workspace needs no network. **Not built yet**: needs the crates downloaded once with

```
cargo fetch --manifest-path C:\Users\User\Projects\kotor-rs\gpu\Cargo.toml
```

Then `cargo build --offline --manifest-path gpu\Cargo.toml`. `scripts/check.ps1` builds and lints this package too as soon as
`gpu/Cargo.lock` exists.
