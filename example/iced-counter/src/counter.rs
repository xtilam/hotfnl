use hotfnl::{hot_check, hot_impl, hot_method};
use iced::widget::{button, column, text};
use iced::*;

#[hot_check]
pub struct Counter {
  value: i64,
  string_view: CustomRender<String>,
  number_view: CustomRender<i32>,
  #[dev]
  is_patching: bool,
}

///
/// # Vi
///
/// **Cảnh báo về ABI Stability / Hot-Reloading:**
///
/// Các variant hệ thống này BẮT BUỘC phải luôn nằm ở các vị trí đầu tiên của enum.
///
/// Khi thực hiện hot-patching (load dynamic library mới), compiler `rustc` không đảm bảo
/// thứ tự gán giá trị (discriminant values) cho các enum variant mặc định (`repr(Rust)`).
///
/// Việc thay đổi thứ tự variant giữa các lần build sẽ làm lệch byte repr trong memory,
/// dẫn đến việc đọc/cast dữ liệu memory trực tiếp (transmute/memory read) giữa Main Binary
/// và Dylib bị sai variant hoặc gây Undefined Behavior (UB).
///
/// # Recommended Fix
///
/// Thay vì phụ thuộc vào thứ tự khai báo thủ công, có thể gán explicit discriminant
/// kết hợp với `#[repr(u8)]` hoặc `#[repr(C)]`:
///
/// ```rust
/// #[repr(u8)]
/// pub enum Message {
///     Rebuild = 0,
///     PatchOk = 1,
///     // ...
/// }
/// ```
#[hot_check]
#[derive(Debug, Clone, Copy)]
pub enum Message {
  #[dev]
  Rebuild,
  #[dev]
  PatchSuccess,
  #[dev]
  PatchFailed,
  Increment,
  Decrement,
}

#[hot_check]
#[hot_impl]
impl Counter {
  pub fn boot() -> (Self, Task<Message>) {
    (
      Self {
        value: 0,
        string_view: CustomRender::new("Hello".to_string()),
        number_view: CustomRender::new(42),
        #[dev]
        is_patching: false,
      },
      Task::none(),
    )
  }

  #[hot_method]
  pub fn update(&mut self, message: Message) {
    match message {
      Message::Increment => {
        self.value += 4;
      }
      Message::Decrement => {
        self.value -= 2;
      }
      #[dev]
      Message::Rebuild => {
        self.is_patching = true;
      }
      #[dev]
      Message::PatchSuccess | Message::PatchFailed => {
        self.is_patching = false;
      }
    }
  }

  #[hot_method]
  pub fn view(&self) -> Element<'_, Message> {
    #[dev]
    if self.is_patching {
      return text("Rebuilding...")
        .size(50)
        .width(Length::Fill)
        .center()
        .into();
    };
    let str = self.string_view.view();
    let number = self.number_view.view();
    column![
      button("Increment").on_press(Message::Increment),
      text(self.value).size(20),
      button("Decrement").on_press(Message::Decrement),
      str,
      number,
    ]
    .width(Length::Fill)
    .padding(20)
    .align_x(Alignment::Center)
    .into()
  }
}

#[derive(Clone)]
pub struct CustomRender<T> {
  data: T,
}

impl<T: std::fmt::Debug> CustomRender<T> {
  pub fn new(data: T) -> Self {
    Self { data }
  }
  /// Renders the custom view using the latest dynamic version of this method.
  ///
  /// # Vi
  ///
  /// Trả về `Element` đại diện cho UI.
  ///
  /// Khi dùng với hot-reloading (patching binary), hàm này luôn gọi phiên bản code
  /// mới nhất vì nó được gọi trực tiếp bên trong `Counter::view`.
  ///
  /// # Safety & Caveats
  ///
  /// **Cảnh báo:** Đây là cơ chế lờ đờ (hacky) và **không an toàn**. Cần cẩn thận khi sử dụng.
  ///
  /// # Technical Details
  ///
  /// * **Generic Function Limitations:** Một hàm generic không có function pointer (fn ptr)
  ///   cố định, nên về mặt kỹ thuật không thể lưu nó vào một jump table để so sánh giữa
  ///   các phiên bản hot-patch.
  /// * **Mangle / Monomorphization Issue:** Ngoại trừ việc chủ động monomorphize thủ công
  ///   (ví dụ: `hello::<String>`) hoặc dùng macro kiểu `CustomRender::new!(hello::<String>)`,
  ///   compiler không thể collect toàn bộ generic types trước.
  /// * **Dynamic Library Unloading:** Ngay cả khi khai báo thủ công, không có gì đảm bảo
  ///   `CustomRender::<T>::view` sẽ tồn tại ở lần build tiếp theo khi reload dylib.
  /// * **Conclusion:** Do đó, việc gọi trực tiếp qua luồng execution của `Counter::view`
  ///   là cách duy nhất để đảm bảo luôn dispatch đúng phiên bản generic fn mới nhất.
  fn view(&self) -> Element<'_, Message> {
    text(format!("Custom Render: {:?}", self.data))
      .size(20)
      .width(Length::Fill)
      .center()
      .into()
  }
}
