use hotfnl::{hot_check, hot_impl, hot_method};
use iced::widget::{button, column, text};
use iced::*;

#[hot_check]
#[derive(Default)]
pub struct Counter {
  value: i64,
  #[dev]
  is_patching: bool,
}

#[hot_check]
#[derive(Debug, Clone, Copy)]
pub enum Message {
  Increment,
  Decrement,
  #[dev]
  Rebuild,
  #[dev]
  PatchOk,
}

#[hot_check]
#[hot_impl]
impl Counter {
  pub fn boot() -> (Self, Task<Message>) {
    (Self::default(), Task::none())
  }

  #[hot_method]
  pub fn update(&mut self, message: Message) {
    match message {
      Message::Increment => {
        self.value += 2;
      }
      Message::Decrement => {
        self.value -= 2;
      }
      #[dev]
      Message::Rebuild => {
        self.is_patching = true;
      }
      #[dev]
      Message::PatchOk => {
        self.is_patching = false;
      }
    }
  }

  #[dev]
  pub fn hot_view(&self) -> Element<'_, Message> {
    match self.is_patching {
      true => text("Patching...")
        .size(50)
        .width(Length::Fill)
        .center()
        .into(),
      false => self.view(),
    }
  }

  #[hot_method]
  pub fn view(&self) -> Element<'_, Message> {
    column![
      button("Increment").on_press(Message::Increment),
      text(self.value).size(20),
      button("Decrement").on_press(Message::Decrement),
    ]
    .width(Length::Fill)
    .padding(20)
    .align_x(Alignment::Center)
    .into()
  }
}
