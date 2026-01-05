use core::{future::Future, pin::Pin};
use alloc::boxed::Box;
use core::task::{Context, Poll};
pub mod keyboard;

pub mod simple_executor; // Мы создадим его на следующем шаге

// Определение структуры Task
// Это "коробка", в которой лежит асинхронная функция
pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    // Создаем новую задачу
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }

    // Метод, чтобы "пнуть" задачу и заставить её поработать
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}