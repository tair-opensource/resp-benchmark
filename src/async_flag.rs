use std::sync::Arc;
use tokio::sync::watch;

pub struct AsyncFlag {
    receiver: watch::Receiver<bool>,
    sender: Arc<watch::Sender<bool>>,
}

impl AsyncFlag {
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(false);
        AsyncFlag { receiver, sender: Arc::new(sender) }
    }

    pub async fn wait_flag(&mut self) {
        self.receiver.changed().await.unwrap();
    }

    pub fn set_flag(&self) {
        self.sender.send(true).unwrap();
    }
}

impl Clone for AsyncFlag {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
            sender: self.sender.clone(),
        }
    }
}
