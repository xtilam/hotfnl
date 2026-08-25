use crate::hotreload::{hotlib::HotLib, hotproject::HotProject};
use anyhow::Result;
use crossterm::terminal::disable_raw_mode;

pub fn app(data: &str) -> Result<()> {
  disable_raw_mode()?;
  let mut project: HotProject = toml::de::from_str(data).unwrap();
  // HotLib::get_instance_mut().config(vec![]);
  project.is_watch_mode = true;
  project.run()?;
  Ok(())
}
