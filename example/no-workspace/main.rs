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
  println!("Hello, world! 1234");
}
