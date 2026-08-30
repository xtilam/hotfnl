use crate::tui::{RComponent, Tui, TuiAppAction};
use anyhow::Result;
use crossterm::event::KeyEvent;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use hotfnl::{hot_impl, hot_main, hot_method};
use ratatui::Frame;
use ratatui::widgets::{Block, Paragraph};
mod macro_utils;
mod tui;

#[hot_main]
fn main() -> Result<()> {
  hotfnl::watch!(watch("./").recursive("../../src"));
  hotfnl::run!();
  enable_raw_mode()?;
  execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
  let app = App::default();
  let mut tui = Tui::new()?;
  tui.tui.clear()?;
  tui.run_tui(app)?;
  disable_raw_mode()?;
  execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
  Ok(())
}

#[derive(Default)]
struct App {
  counter: i32,
}

#[hot_impl]
impl RComponent for App {
  #[hot_method]
  fn key_handler(&mut self, key: &KeyEvent, tx: &tui::AppTx) {
    use crossterm::event::KeyCode::*;
    use crossterm::event::KeyModifiers;
    match (key.code, key.modifiers.contains(KeyModifiers::CONTROL)) {
      (Char('q') | Esc, _) | (Char('c'), true) => {
        tx.send(TuiAppAction::Quit).ok();
      }
      (Char('r'), _) => {
        tx.send(TuiAppAction::ReRender).ok();
      }
      (Left, _) => {
        self.counter -= 2;
        tx.send(TuiAppAction::ReRender).ok();
      }
      (Right, _) => {
        self.counter += 2;
        tx.send(TuiAppAction::ReRender).ok();
      }
      _ => {}
    }
  }

  #[hot_method]
  fn render(&self, frame: &mut Frame) {
    use ratatui::prelude::*;
    let size = frame.area();
    let block = Block::default()
      .title(
        Span::from(format!("Counter: {}", 123))
          .bold()
          .underlined()
          .fg(Color::Yellow),
      )
      .borders(ratatui::widgets::Borders::ALL);
    frame.render_widget(
      Paragraph::new(format!("Counter: hello! {}", self.counter))
        .block(block)
        .alignment(Alignment::Center),
      size,
    );
  }
}
