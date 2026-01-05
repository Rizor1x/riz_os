use super::Task;
use alloc::collections::VecDeque; // Очередь (двусвязный список)
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>, // Очередь задач
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
        }
    }

    // Добавить задачу в очередь
    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    // Главный цикл: крутим задачи, пока они есть
    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            // Создаем фиктивный Waker (будильник).
            // Пока что он ничего не делает. Если задача уснет, мы просто снова её опросим.
            // Это неэффективно (процессор будет греться), но для теста сойдет.
            let waker = dummy_waker();
            let mut context = Context::from_waker(&waker);
            
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // Задача завершилась (сделала return). Отлично, забываем про неё.
                }
                Poll::Pending => {
                    // Задача сказала "я жду". Кладем её обратно в конец очереди.
                    self.task_queue.push_back(task);
                }
            }
        }
    }
}

// --- BOILERPLATE CODE (Скучный обязательный код для Rust Async) ---

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const (), vtable)
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}