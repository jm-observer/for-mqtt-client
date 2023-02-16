use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::{channel, Receiver, Sender};

#[derive(Clone)]
pub struct Endpoint<X, Y> {
    tx: Sender<X>,
    rx_tx: Sender<Y>,
}

impl<X: Clone, Y: Clone> Endpoint<X, Y> {
    pub fn channel(buffer: usize) -> (Endpoint<X, Y>, Endpoint<Y, X>) {
        let (tx0, _) = channel(buffer);
        let (tx1, _) = channel(buffer);
        (
            Endpoint {
                tx: tx0.clone(),
                rx_tx: tx1.clone(),
            },
            Endpoint {
                tx: tx1,
                rx_tx: tx0,
            },
        )
    }

    pub fn send(&self, x: X) -> Result<usize, SendError<X>> {
        self.tx.send(x)
    }
    pub fn receiver(&self) -> Receiver<Y> {
        self.rx_tx.subscribe()
    }
}
