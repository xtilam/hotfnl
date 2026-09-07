use hotfnl::{hot_main, match_hot};

use crate::counter::Counter;
mod counter;

#[hot_main]
pub fn main() -> iced::Result {
  hotfnl::watch!(recursive("./").recursive("../../../src/"));
  hotfnl::run!();
  let app = iced::application(
    Counter::boot,
    Counter::update,
    match_hot!({ Counter::hot_view }, { Counter::view }),
  );
  match_hot![{
    let app = app.subscription(|_| {
      use crate::counter::Message::{self, *};
      use iced::{
        Subscription,
        futures::{Stream, channel::mpsc},
        stream,
      };
      fn callback() -> impl Stream<Item = Message> {
        stream::channel(100, async |mut output| {
          let (sender, mut receiver) = mpsc::channel(100);
          hotfnl::use_local_event!(|evt| evt
            .on_source_changed({
              let sender = sender.clone();
              move || {
                let mut sender = sender.clone();
                sender.try_send(Rebuild).ok();
              }
            })
            .on_rebuild_error({
              let sender = sender.clone();
              move || {
                let mut sender = sender.clone();
                sender.try_send(PatchFailed).ok();
                println!("Rebuild failed");
              }
            })
            .on_patch_success({
              let sender = sender.clone();
              move || {
                let mut sender = sender.clone();
                sender.try_send(PatchSuccess).ok();
              }
            }));

          loop {
            use iced::futures::StreamExt;
            let input = receiver.select_next_some().await;
            output.try_send(input).ok();
          }
        })
      }
      Subscription::run(callback)
    });
  }];
  // if_hot!();
  app.run()?;
  Ok(())
}
