use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::watch;

pub struct AsyncFlag {
    receiver: watch::Receiver<bool>,
    sender: Arc<watch::Sender<bool>>,
    flag: Arc<AtomicBool>,
}

impl AsyncFlag {
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(false);
        AsyncFlag { receiver, sender: Arc::new(sender), flag: Arc::new(AtomicBool::new(false)) }
    }

    pub async fn wait_flag(&mut self) {
        if self.flag.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        self.receiver.changed().await.unwrap();
    }

    pub fn set_flag(&self) {
        self.sender.send(true).unwrap();
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn flag(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Clone for AsyncFlag {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
            sender: self.sender.clone(),
            flag: self.flag.clone(),
        }
    }
}
