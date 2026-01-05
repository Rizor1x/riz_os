use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Context, Poll}};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;

// Очередь для байтов мыши
static MOUSE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

pub(crate) fn add_mouse_packet(packet: u8) {
    if let Ok(queue) = MOUSE_QUEUE.try_get() {
        if let Err(_) = queue.push(packet) {
            // Очередь полна, выкидываем старые данные
        } else {
            WAKER.wake();
        }
    }
}

pub struct MouseStream {
    _private: (),
}

impl MouseStream {
    pub fn new() -> Self {
        MOUSE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("MouseStream::new should only be called once");
        MouseStream { _private: () }
    }
}

impl Stream for MouseStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = MOUSE_QUEUE.try_get().expect("not initialized");

        if let Some(packet) = queue.pop() {
            return Poll::Ready(Some(packet));
        }

        WAKER.register(&cx.waker());
        
        match queue.pop() {
            Some(packet) => {
                WAKER.take();
                Poll::Ready(Some(packet))
            }
            None => Poll::Pending,
        }
    }
}