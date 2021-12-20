use std::collections::VecDeque;
use std::intrinsics::unreachable;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar};
use futures::future::BoxFuture;

use common_exception::ErrorCode;
use common_exception::Result;
use common_infallible::Mutex;

use crate::pipelines::new::executor::executor_graph::RunningGraph;
use crate::pipelines::new::executor::executor_worker_context::ExecutorWorkerContext;
use crate::pipelines::new::processors::processor::ProcessorPtr;

pub struct ExecutorTasksQueue {
    finished: AtomicBool,
    workers_tasks: Mutex<ExecutorTasks>,
}

impl ExecutorTasksQueue {
    pub fn create(workers_size: usize) -> ExecutorTasksQueue {
        ExecutorTasksQueue {
            finished: AtomicBool::new(false),
            workers_tasks: Mutex::new(ExecutorTasks::create(workers_size)),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    // Pull task from the global task queue
    pub fn steal_task_to_context(&self, context: &mut ExecutorWorkerContext) {
        unsafe {
            let mut workers_tasks = self.workers_tasks.lock();
            if !workers_tasks.is_empty() {
                let best_worker_id = workers_tasks.best_worker_id(context.get_worker_num());
                // context.task = workers_tasks.pop_task(best_worker_id);

                // if thread.task.is_some() && !workers_tasks.is_empty() {
                //     // TODO:
                // }

                return;
            }
        }

        self.wait_wakeup(context)
    }

    pub fn push_executing_async_task(&self, worker_id: usize, task: ExecutingAsyncTask) -> Option<ExecutingAsyncTask> {
        unsafe {
            let mut workers_tasks = self.workers_tasks.lock();

            // The finished when wait the lock tasks. TODO: maybe use try lock.
            match task.finished.load(Ordering::Relaxed) {
                true => Some(task),
                false => {
                    workers_tasks.push_executing_async_task(worker_id, task);
                    None
                }
            }
        }
    }

    pub fn wait_wakeup(&self, thread: &mut ExecutorWorkerContext) {
        // condvar.wait(guard);
        unimplemented!()
    }
}

pub struct ExecutingAsyncTask {
    pub finished: Arc<AtomicBool>,
    pub future: BoxFuture<'static, Result<()>>,
}

struct ExecutorTasks {
    tasks_size: usize,
    workers_sync_tasks: Vec<VecDeque<ProcessorPtr>>,
    workers_async_tasks: Vec<VecDeque<ProcessorPtr>>,
    workers_executing_async_tasks: Vec<VecDeque<ExecutingAsyncTask>>,
}

unsafe impl Send for ExecutorTasks {}

impl ExecutorTasks {
    pub fn create(workers_size: usize) -> ExecutorTasks {
        let mut workers_sync_tasks = Vec::with_capacity(workers_size);
        let mut workers_async_tasks = Vec::with_capacity(workers_size);
        let mut workers_executing_async_tasks = Vec::with_capacity(workers_size);

        for _index in 0..workers_size {
            workers_sync_tasks.push(VecDeque::new());
            workers_async_tasks.push(VecDeque::new());
            workers_executing_async_tasks.push(VecDeque::new());
        }

        ExecutorTasks {
            tasks_size: 0,
            workers_sync_tasks,
            workers_async_tasks,
            workers_executing_async_tasks,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tasks_size == 0
    }

    pub unsafe fn best_worker_id(&mut self, mut priority_works_id: usize) -> usize {
        for _index in 0..self.workers_sync_tasks.len() {
            if !self.workers_sync_tasks[priority_works_id].is_empty() {
                break;
            }

            priority_works_id += 1;
            if priority_works_id >= self.workers_sync_tasks.len() {
                priority_works_id = 0;
            }
        }

        priority_works_id
    }

    pub unsafe fn pop_task(&mut self, worker_id: usize) -> Option<ProcessorPtr> {
        self.tasks_size -= 1;
        self.workers_sync_tasks[worker_id].pop_front()
    }

    pub unsafe fn push_executing_async_task(&mut self, worker_id: usize, task: ExecutingAsyncTask) {
        self.tasks_size += 1;
        self.workers_executing_async_tasks[worker_id].push_back(task)
    }
}
