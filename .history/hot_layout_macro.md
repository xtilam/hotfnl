# `#[hot_layout]` — phát hiện layout struct thay đổi

## Mục tiêu

Thêm proc macro `hot_layout` gắn vào struct. Lúc boot, host thu thập hash nội dung
struct (`file:struct_name`) làm baseline. Khi load `.so` mới, so sánh hash với baseline;
nếu phát hiện layout struct bị thay đổi (thêm/bớt/sửa field, đổi repr, ...) thì app
`exit(0)` để wrapper respawn lại binary vừa build (vì binary mới có struct layout mới).

## Cơ chế

- Y hệt so sánh fn hiện tại: `BTreeMap<String, u64>`, key = `file:struct_name`, value =
  hash **u64** của toàn bộ struct-def (tính deterministic ở compile-time trong proc macro
  bằng FNV-1a 64 over `quote!(#item).to_string()`).
- Thu thập qua `inventory`, giống `HotFn`: struct `crate::hot::HotLayout` được
  `inventory::collect!` trong module `hot` do `#[hot_main]` sinh ra; `.so` export
  `hrl_get_layouts() -> Vec<hotfnl::HotLayout>`.
- Mismatch / thừa / thiếu key → `PatchErr::ToManyChange` (dùng lại, KHÔNG thêm variant
  mới) → `run_watch_lib` đã `std::process::exit(0)` sẵn.

## Thay đổi chi tiết

### 1. `packages/hotfnl-proc-macro/lib.rs`

- **Thêm `#[hot_layout]`**:
  - `prod` → no-op (trả nguyên item).
  - Dev: parse `ItemStruct`, lấy `struct_name` (ident đầu tiên), hash FNV-1a 64 of
    `quote!(#item).to_string()`, emit:
    ```rust
    #item
    hotfnl::inventory::submit! {
      crate::hot::HotLayout {
        file_name: file!(),
        struct_name: "Foo",
        hash: <u64>,
      }
    }
    ```
- **Sửa `#[hot_main]`**:
  - Module `hot`: thêm `pub struct HotLayout { pub file_name: &'static str, pub
    struct_name: &'static str, pub hash: u64 }`.
  - Thêm `hotfnl::inventory::collect!(crate::hot::HotLayout);`.
  - Thêm export `hrl_get_layouts() -> Vec<hotfnl::HotLayout>`.
  - Boot call truyền thêm `Vec<HotLayout>` thu thập từ inventory.

### 2. `src/hotreload/hotfn.rs`

Thêm:
```rust
pub struct HotLayout {
  pub file_name: &'static str,
  pub struct_name: &'static str,
  pub hash: u64,
}
```

### 3. `src/hotreload/hotlib.rs`

- `HotLib` thêm field `pub layout_dict: BTreeMap<String, u64>` (+ `Default`).
- `on_boot(...)` nhận thêm `layout: Vec<HotLayout>`, build dict (dup key → panic).
- `get_lib(...)`: resolve `hrl_get_layouts` (optional — lib cũ không export → skip), so
  như fn; mismatch/thừa/thiếu key → close lib → `Err(PatchErr::ToManyChange)`.

### 4. `src/hotreload.rs`

- `boot()` thêm tham số `layout: Vec<HotLayout>` truyền vào `on_boot`.

### 5. `example/iced-counter`

- Gắn `#[hot_layout]` vào `counter` struct.

### 6. Docs

- `.context/hotlib.md`, `README.md`: thêm `#[hot_layout]` vào bảng API / flow.

## Verify

- `cargo build` (default). Không test runtime.

## Ghi chú

- `HotLayout` dùng `u64` cho hash (quyết định cuối cùng).
- Không thêm variant `PatchErr::LayoutChanged` — tái dùng `ToManyChange`.
- hash tính override struct-def tại expansion (inventory cần const literal); filename nằm
  trong map key.