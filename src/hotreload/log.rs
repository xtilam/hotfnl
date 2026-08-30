use std::io::Write;
use std::process::Stdio;

use crate::hotreload::hotproject::HotProject;

pub struct HotLogger {
  file: std::fs::File,
}

impl TryFrom<&HotProject> for HotLogger {
  type Error = std::io::Error;
  fn try_from(value: &HotProject) -> Result<Self, Self::Error> {
    let log_path = value.files().data().log_path();
    let file = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(log_path)?;
    Ok(Self { file })
  }
}

#[allow(unused)]
impl HotLogger {
  pub fn write(&mut self, msg: &str) {
    let _ = write!(self.file, "{}", msg);
  }
  pub fn writeln(&mut self, msg: &str) {
    let _ = writeln!(self.file, "{}", msg);
  }
  pub fn write_section(&mut self, section: &str) {
    static LINE: &str = "==================================================";
    writeln!(self.file, "{}\n{}\n{}", LINE, section, LINE).ok();
  }
  pub fn stdio(&self) -> Stdio {
    Stdio::from(self.file.try_clone().unwrap())
  }
}
