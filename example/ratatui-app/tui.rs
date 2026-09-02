use crossterm::event::KeyEvent;
use anyhow::Result;
use ratatui::{Frame, Terminal, backend::CrosstermBackend};

use crate::macro_utils::macro_utils::bselect;

pub enum TuiAppAction {
  ReRender,
  Quit,
}

pub type AppTx = crossbeam_channel::Sender<TuiAppAction>;
pub struct Tui {
  pub tui: Terminal<CrosstermBackend<std::io::Stdout>>,
}
impl Tui {
  pub fn new() -> Result<Self> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let tui = Terminal::new(backend)?;
    Ok(Self { tui })
  }
  pub fn run_tui(&mut self, mut app: impl RComponent) -> Result<()> {
    use TuiAppAction::*;
    let (tx, rx_render) = crossbeam_channel::unbounded::<TuiAppAction>();

    hotfnl::use_local_event!(|e| e.on_patch_success({
      let tx = tx.clone();
      move || {
        tx.send(ReRender).ok();
      }
    }));
    let (tx_term, rx_term) = crossbeam_channel::unbounded();
    tx.send(ReRender).ok();
    std::thread::spawn({
      let tx = tx_term.clone();
      move || -> Result<()> {
        loop {
          if let Ok(key) = crossterm::event::read() {
            if let crossterm::event::Event::Key(key) = key {
              tx.send(key)?;
            }
          }
        }
      }
    });
    loop {
      bselect!(
        [recv(rx_render), |action| {
          let action = action?;
          match action {
            ReRender => {
              self.tui.draw(|frame| {
                app.render(frame);
              })?;
            }
            Quit => break,
          };
        }],
        [recv(rx_term), |key| {
          let key = key?;
          app.key_handler(&key, &tx);
        }]
      );
    }
    Ok(())
  }
}

pub trait RComponent {
  fn key_handler(&mut self, key: &KeyEvent, tx: &AppTx) {
    let _ = key;
    let _ = tx;
  }
  fn render(&self, frame: &mut Frame);
}
