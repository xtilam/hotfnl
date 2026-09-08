# hotfnl

A lightweight **hot function swapping** library for Rust binary applications on Linux, designed for **development-time** use.

![Image](https://github.com/user-attachments/assets/34e6f15e-e629-4a8b-b431-085f7e49deeb)

`hotfnl` works by compiling your project into a dynamic library (`.so`), loading it at
runtime, and swapping the function pointers of annotated functions — all without
restarting your application. When a source file changes, the library is recompiled and
hot-swapped in place.

> [!CAUTION]
> This is inherently **unsafe**: swapping live function pointers requires the new
> implementation to be byte-compatible with the original. No ABI guarantee is enforced.
> Only **Linux** is supported. This library is for **Rust binary applications** and
> intended for **development iteration only** — not for production use. When shipping,
> enable the `prod` feature to strip out all hot-reload APIs with zero runtime overhead.

## Feature flags

| Feature   | Effect                                                                                                                                          |
|-----------|-------------------------------------------------------------------------------------------------------------------------------------------------|
| `default` | Enables all hot-reloading machinery: proc-macro expansion, file watcher, dynamic library loading, wrapper process, and the event system.        |
| `prod`    | All proc macros (`#[hot_main]`, `#[hot_fn]`, `#[hot_impl]`, `#[hot_method]`) become pass-through no-ops, and the hot-reload runtime is not compiled at all. Zero runtime overhead. |

> [!NOTE]
> When both `default` and `prod` are enabled, `prod` takes precedence by turning all
> proc macros into no-ops. For item-level conditional compilation, pair `#[hot_check]` with
> `#[dev]` / `#[prod]` attributes, which are rewritten into the correct `cfg` gates in
> whichever mode you build.

## Quick start

Add `hotfnl` to your dependencies (default features without `prod`):

```toml
[dependencies]
hotfnl = "0.1"
```

Annotate your `main` with `#[hot_main]` and the functions you want to hot-swap with
`#[hot_fn]`:

```rust
use hotfnl::{hot_fn, hot_main};

#[hot_main]
fn main() {
  hotfnl::watch!(watch("./"));
  hotfnl::run!(); // the wrapper process is executed here
  loop {
    std::thread::sleep(std::time::Duration::from_secs(1));
    hello();
  }
}

#[hot_fn]
fn hello() {
  println!("Hello, world!");
}
```

On first launch the app is scaffolded, built, and run through a wrapper binary. From then
on, saving a source file triggers a rebuild and a live function-pointer swap — the app
keeps running. See [How it works](#how-it-works) for the full lifecycle.

## Installation

Add `hotfnl` as an **optional** dependency and expose it through feature flags:

```toml
[dependencies]
hotfnl = { version = "0.1", optional = true }

[features]
default = ["hotfnl/default"]
prod = ["hotfnl/prod"]
```

| Command                            | Mode  | Effect                                        |
|------------------------------------|-------|-----------------------------------------------|
| `cargo run`                        | Dev   | Hot-reload enabled (default features).         |
| `cargo build --release -F prod`    | Prod  | All proc macros become no-ops, zero runtime overhead. |

## How it works

### Boot phase

On first launch, `#[hot_main]` checks the `HOT_PROJECT_DIR` environment variable. If it's
set, the binary was built by the generated hot project and scaffolding is skipped. If not,
`hotfnl::run!()` scaffolds a "hot project" into `target/hotfnl/<bin-name>/`:

- The project source is cloned and its `Cargo.toml` rewritten with absolute dependency
  paths, added `[lib] crate-type=["cdylib"]`, and two extra `[[bin]]` entries.
- `Cargo.lock` is **copied** (not symlinked) into the hot project. This avoids a build lock
  when `cargo build` runs while the main binary is also being managed, and lets the hot
  project share the main binary's `target` directory instead of inflating it.
- A `project_data.toml` is written under `target/<profile>/hotfnl/<bin-name>/` as the hand-off
  for the wrapper.

Three build targets are produced:

| Target               | Role                                                                                              |
|----------------------|---------------------------------------------------------------------------------------------------|
| `hotfnlw_<bin>`      | The **wrapper**: watches sources, triggers rebuilds, restarts the app, and reports build state over a Unix socket. |
| `hotfnl_<bin>`       | The **app binary** that actually runs; boots with `is_hot_project = true`.                        |
| `libhotfnl_<bin>.so` | The hot-swappable **cdylib**; exports `hrl_get_functions` and carries the latest function implementations. |

After the initial build, the process `exec`s itself into the wrapper, which in turn spawns
the app binary.

### Runtime phase

The wrapper watches your source files. On a change it re-runs `cargo build`; a successful
build is snapshot as a versioned `.so` under `data/lib/lib_<version>.so`. The wrapper sends
states (`SourceChanged`, `Rebuilding`, `BuildSuccess(version)`, `BuildFailed`) to the app
over a Unix datagram socket.

When the app receives `BuildSuccess`, it `dlopen`s the new `.so`, resolves `hrl_get_functions`
to gather the new function set, and swaps them into the shared function table, so the next
call dispatches to the latest version. Each annotated `#[hot_fn]` is rewritten into a
`#[no_mangle]` stub that dispatches through the shared table.

Because swapping is `unsafe` and a hot function may crash (e.g. `SIGSEGV`), the wrapper
supervises the app: if the app exits, it respawns it. After each successful rebuild it
allows one respawn, and if the app does not come back up it waits for the next source
change.

## API overview

| Item                                          | Purpose                                                         |
|-----------------------------------------------|-----------------------------------------------------------------|
| `#[hot_main]`                                 | Wrap `main` to bootstrap the hot-reload system.                 |
| `#[hot_fn]`                                   | Make a free function hot-patchable.                             |
| `#[hot_impl]` + `#[hot_method]`               | Make an associated method hot-patchable.                        |
| `#[hot_check]`                                | Rewrite `#[dev]`/`#[prod]` attributes to proper `cfg` gates.   |
| `hotfnl::run!()`                              | Start the hot-reload runtime.                                   |
| `hotfnl::watch!(watch("./src"))`              | Watch extra paths (use `recursive(...)` for recursive watch).   |
| `hotfnl::use_event!()` / `use_local_event!()` | Register lifecycle callbacks (`on_pre_patch`, `on_patch_success`, `on_patch_error`, `on_clean_up`, `on_source_changed`). |
| `hotfnl::use_cargo_args!()`                   | Pass additional cargo arguments to the hot-reload build.       |
| `hotfnl::if_hot!()` / `if_prod!()`            | Conditional compilation: emit code only in hot or prod builds.  |

## Examples

- [`example/no-workspace`](example/no-workspace) — minimal standalone (non-workspace) app.
- [`example/ratatui-app`](example/ratatui-app) — a `ratatui` TUI app using methods.
- [`example/iced-counter`](example/iced-counter) — an `iced` GUI counter app.

> [!WARNING]
> Generic functions are **not** supported for hot-reload. A generic function has no fixed
> function pointer and cannot be stored in a jump table for patching across versions.
> Use non-generic `#[hot_fn]` / `#[hot_method]` functions as entry points, and call
> generic code from within them. See
> [`example/iced-counter/src/counter.rs` — `CustomRender::view`](example/iced-counter/src/counter.rs)
> for a detailed explanation and workaround.

## Requirements

- Linux only (`hotfnl` uses `.so` dynamic libraries and `exec`).
- Rust edition 2024.
- Intended for **development-time** use only. For production, build with
  `-F prod`.

## License

Licensed under the [MIT License](LICENSE).
