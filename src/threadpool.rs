//! Thread pool! A pool of threads ready to do work without needing to spawn new threads
//! for every task. Uses mpsc channels.

use std::sync::mpsc;
use std::thread;

type Task = Box<dyn FnOnce() + Send>;

enum OrderToManager {
	/// The manager will forward the given task to an available worker immediately
	/// or as soon as a worker becomes available.
	Task(Task),
	/// The manager will ask all workers to end, and then follow them to where
	/// dead threads go, hopefully a beautiful and peaceful place.
	_End,
}

pub struct ThreadPool {
	order_sender_to_manager: mpsc::Sender<OrderToManager>,
	number_of_workers: usize,
}

impl ThreadPool {
	/// Creates a pool of worker threads ready to work for us. We can send tasks for them to run,
	/// without the overhead of spawning threads (which makes the OS do quite some work) for each
	/// new task to be done.
	///
	/// There will be `number_of_workers` worker threads, and one additional manager thread.
	///
	/// Sending tasks to the thread pool enqueues them, and first sent tasks are given to workers
	/// as soon as possible (immediately or when workers become available after completing previous
	/// tasks). It works!
	///
	/// TODO: Maybe add a way to make sure that some workers are kept available for some kinds of
	/// tasks, for example we want remeshing of chunks where the player places a block to be
	/// completed as soon as possible, but generation of stuff could be taking all the workers
	/// and a ot of place ahead in the pending task queue, so we would want the high priority
	/// remeshing tasks to skip the whole queue and maybe even not wait for a worker to finish
	/// by being handled by dedicated workers.
	pub fn new(number_of_workers: usize) -> ThreadPool {
		// The threadpool owner can order the manager around via this channel.
		let (order_sender_to_manager, manager_order_receiver) = mpsc::channel::<OrderToManager>();

		// Here we spawn the manager thread.
		// We do not need to keep the `JoinHandle`, we can just send it an `OrderToManager::End`
		// to make it end its thread (after making sure all the workers also end).
		thread::spawn(move || {
			// When a worker is done with its task (or has just spawned), it tells the manager
			// so that the manager can give them a new task, maybe immediately.
			// Each worker will take a clone of the sender.
			let (asking_for_more_sender_to_manager, manager_receiver_of_worker_asking_for_more) =
				mpsc::channel::<WorkerId>();
			type WorkerId = usize;

			enum OrderToWorker {
				/// The worker will perform the given task, and then ask for more.
				Task(Task),
				/// The worker will end its loop, letting its thread rest in piece, free at last.
				End,
			}

			// Here we spawn the desired number of worker threads.
			// The manager will keep an array of senders, one for each worker, so that the manager can
			// order around every worker.
			// We do not need to keep the `JoinHandle`s, the manager can just send them
			// `OrderToWorker::End` to make them end their threads.
			let mut order_sender_to_worker_array = Vec::with_capacity(number_of_workers);
			for worker_id in 0..number_of_workers {
				let (order_sender_to_worker, worker_order_receiver) = mpsc::channel::<OrderToWorker>();
				order_sender_to_worker_array.push(order_sender_to_worker);
				let asking_for_more_sender_to_manager = asking_for_more_sender_to_manager.clone();
				thread::spawn(move || loop {
					// Either we (a worker) just spawned (and thus are ready to be given a first task)
					// or we just finished, in either case we are ready to take the next order.
					let manager_is_gone = asking_for_more_sender_to_manager.send(worker_id).is_err();
					if manager_is_gone {
						// If the manager is gone, then we are free to go as well.
						return;
					}

					// The manager gave an order, so we obey.
					let order = worker_order_receiver.recv();
					match order {
						Ok(OrderToWorker::Task(task)) => task(),
						Ok(OrderToWorker::End) | Err(_) => return,
					}
				});
			}

			// We don't keep a clone of this sender, clones of this are for workers only.
			drop(asking_for_more_sender_to_manager);

			// Setup is done, now as the manager we enter a loop in which we dispatch tasks
			// to available workers until the end. We live in a society.
			loop {
				let order = manager_order_receiver.recv();
				match order {
					Ok(OrderToManager::Task(task)) => {
						let worker_asking_for_more =
							manager_receiver_of_worker_asking_for_more.recv().unwrap();
						order_sender_to_worker_array[worker_asking_for_more]
							.send(OrderToWorker::Task(task))
							.unwrap();
					},
					Ok(OrderToManager::_End) | Err(_) => {
						for worker_asking_for_more in manager_receiver_of_worker_asking_for_more.iter() {
							order_sender_to_worker_array[worker_asking_for_more]
								.send(OrderToWorker::End)
								.unwrap();
						}
						return;
					},
				}
			}
		});

		ThreadPool { order_sender_to_manager, number_of_workers }
	}

	/// Ends the manager and worker threads.
	/// Note that dropping the `ThreadPool` should do the trick too (as it hangs up a channel
	/// that makes the manager behaves the same way it would as by calling this method).
	pub fn _end(self) {
		self
			.order_sender_to_manager
			.send(OrderToManager::_End)
			.unwrap();
	}

	pub fn enqueue_task(&self, task: Task) {
		self
			.order_sender_to_manager
			.send(OrderToManager::Task(task))
			.unwrap();
	}

	pub fn number_of_workers(&self) -> usize {
		self.number_of_workers
	}
}
