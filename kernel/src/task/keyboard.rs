use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Context, Poll}};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;

// Очередь для скан-кодов. Размер 100 - хватит даже для очень быстрого тайпинга.
// ArrayQueue - супер-быстрая lock-free очередь.
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

// Waker - штука, которая будет будить Executor, когда придет байт.
static WAKER: AtomicWaker = AtomicWaker::new();

/// Вызывается из ПРЕРЫВАНИЯ (interrupts.rs). Должна работать мгновенно.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            // Очередь полна. Игнорируем ввод (или можно пищать динамиком)
            crate::serial_println!("WARNING: Scancode queue full; dropping input");
        } else {
            // ГЛАВНЫЙ МОМЕНТ: Будим задачу, которая ждет этот скан-код!
            WAKER.wake();
        }
    } else {
        crate::serial_println!("WARNING: Scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    _private: (), // Чтобы нельзя было создать структуру извне
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

// Реализуем Stream - это как Iterator, только асинхронный.
impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("not initialized");

        // 1. Пытаемся достать байт из очереди
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        // 2. Если пусто - регистрируем Waker (говорим: "Разбуди меня, когда что-то появится")
        WAKER.register(&cx.waker());
        
        // 3. Проверяем еще раз (на случай гонки данных, если прерывание случилось ПРЯМО СЕЙЧАС)
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending, // Спим
        }
    }
}