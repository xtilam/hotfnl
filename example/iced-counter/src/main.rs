use hotfnl::{hot_main, match_hot};

use crate::counter::Counter;
mod counter;

#[hot_main]
pub fn main() -> iced::Result {
  hotfnl::watch!(recursive("./").recursive("../../../src/"));
  hotfnl::run!();
  let app = iced::application(Counter::boot, Counter::update, Counter::view);
  match_hot![{
    let app = app.subscription(|_| {
      use crate::counter::Message::{self};
      use iced::{
        Subscription,
        futures::{Stream, channel::mpsc},
        stream,
      };
      fn callback() -> impl Stream<Item = Message> {
        stream::channel(100, async |mut output| {
          let (sender, mut receiver) = mpsc::unbounded::<hotfnl::EventType>();
          hotfnl::use_local_event!(event_ptr, {
            let sender = sender.clone();
            move |event| {
              sender.unbounded_send(event).ok();
            }
          });
          loop {
            use iced::futures::StreamExt;
            match receiver.select_next_some().await {
              SourceChanged | StartRebuild => output.try_send(Message::Rebuild).ok(),
              PatchSuccess | PatchError => output.try_send(Message::PatchSuccess).ok(),
              _ => Some(()),
            };
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
