# hotfnl

A lightweight **hot function swapping** library for Rust applications on Linux.

`hotfnl` lets you replace individual function implementations at runtime without
restarting your application. When a source file changes, `hotfnl` recompiles the project
into a dynamic library (`.so`), loads it, and swaps the function pointers of the functions
you opt into.

> [!NOTE]
> This is **not** full module hot-reloading (HMR). It only swaps function pointers for
> annotated functions. It is inherently `unsafe`: the new library must contain functions
> with byte-compatible signatures, and no ABI guarantee is enforced. Use it for
> development-time iteration loops, not in production.

## Feature flags

| Feature   | Effect                                                             |
|-----------|--------------------------------------------------------------------|
| `default` | Enables all hot-reloading machinery and proc-macro expansion.      |
| `prod`    | Strips all hot-reloading code; zero runtime overhead.              |

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
  hotfnl::run!();
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

On first launch, `hotfnl` scaffolds a "hot project" (a generated Cargo project alongside
your source), builds it, and runs your app through a wrapper binary. From then on, saving
a source file triggers a rebuild and a live function-pointer swap — the app keeps running.

## API overview

| Item                                    | Purpose                                                         |
|-----------------------------------------|-----------------------------------------------------------------|
| `#[hot_main]`                           | Wrap `main` to bootstrap the hot-reload system.                 |
| `#[hot_fn]`                             | Make a free function hot-patchable.                             |
| `#[hot_impl]` + `#[hot_method]`         | Make an associated method hot-patchable.                        |
| `hotfnl::run!()`                        | Start the hot-reload runtime.                                   |
| `hotfnl::watch!(watch("./src"))`        | Watch extra paths (use `recursive(...)` for recursive watch).   |
| `hotfnl::use_event!()` / `use_local_event!()` | Register lifecycle callbacks (`on_pre_patch`, `on_patch_success`, `on_patch_error`, `on_clean_up`). |

## Examples

- [`example/no-workspace`](example/no-workspace) — minimal standalone (non-workspace) app.
- [`example/ratatui-app`](example/ratatui-app) — a `ratatui` TUI app using methods.

## Requirements

- Linux (`hotfnl` uses Unix symlinks, `.so` dynamic libraries, and `exec`).
- Rust edition 2024.

## License

Licensed under the [MIT License](LICENSE).
