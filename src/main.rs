use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

// The kind of task, which also dictates its CPU footprint
#[derive(Debug, Clone, Copy)]
pub enum TaskKind {
    IO,
    CPU,
}

impl TaskKind {
    // Hardcoded CPU percentage limits based on the amendments
    pub fn cpu_cost(&self) -> u32 {
        match self {
            TaskKind::IO => 10,
            TaskKind::CPU => 35,
        }
    }
}

// The core task structure
#[derive(Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub kind: TaskKind,
    pub arrival_time: Duration,
    pub duration: Duration, // Will always be 200ms based on amendments
}

//Task Generator
pub fn generate_tasks(total_tasks: u32, io_probability: f64) -> Vec<Task> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut tasks = Vec::with_capacity(total_tasks as usize);

    for i in 0..total_tasks {
        // Roll the dice to determine if the task is IO or CPU
        let kind = if rng.gen_bool(io_probability) {
            TaskKind::IO
        } else {
            TaskKind::CPU
        };

        tasks.push(Task {
            id: i,
            kind,
            // 20ms interval per task (0ms, 20ms, 40ms, etc.)
            arrival_time: Duration::from_millis((i * 20) as u64),
            // Both CPU and IO tasks take 200ms according to the amendments
            duration: Duration::from_millis(200),
        });
    }
    tasks
}

// Shared State for the Manager and Workers
#[derive(Debug)]
pub struct SystemState {
    pub active_workers: u32,
    pub current_cpu_usage: u32,
}

impl SystemState {
    pub fn new() -> Self {
        Self {
            active_workers: 0,
            current_cpu_usage: 0,
        }
    }
    pub fn can_dispatch(&self, task: &Task) -> bool {
        let has_worker = self.active_workers < 8;
        let has_cpu = (self.current_cpu_usage + task.kind.cpu_cost()) <= 100;

        has_worker && has_cpu
    }
}

fn main() {
    println!("Concurrent Task Dispatcher initialized.");

    // Generate and Sort Tasks
    let mut generated_tasks = generate_tasks(1000, 0.70);
    // Sort task by arrival time just in case
    generated_tasks.sort_by_key(|t| t.arrival_time);

    // Quick verification to ensure randomizer is working
    let io_count = generated_tasks.iter().filter(|t| matches!(t.kind, TaskKind::IO)).count();
    let cpu_count = generated_tasks.len() - io_count;
    println!(
        "Generated {} tasks: {} IO, {} CPU",
        generated_tasks.len(),
        io_count,
        cpu_count
    );

    // Load them into the Manager's Queue
    let mut manager_queue: VecDeque<Task> = VecDeque::from(generated_tasks);

    // Create the global shared state, protected by an Arc and Mutex
    let shared_state = Arc::new(Mutex::new(SystemState::new()));

    //println!("Loaded {} tasks into the manager queue.", manager_queue.len());
    //println!("Initial System State: {:?}", shared_state.lock().unwrap());

    // The Worker Pool

    // Create the channel for the Manager to send tasks to Workers
    let (tx, rx) = mpsc::channel::<Task>();
    let shared_rx = Arc::new(Mutex::new(rx));

    // Spawn exactly 8 worker threads
    let mut worker_handles = vec![];

    for _ in 0..8 {
        let rx_clone = Arc::clone(&shared_rx);
        let state_clone = Arc::clone(&shared_state);

        let handle = thread::spawn(move || {
            loop {
                // Safely grab the next task from the channel
                let task = {
                    let lock = rx_clone.lock().unwrap();
                    match lock.recv() {
                        Ok(t) => t,
                        Err(_) => break, // If the channel closes, the thread shuts down cleanly
                    }
                };

                // Simulate executing the task
                thread::sleep(task.duration);

                // Task is done! Release the resources back to the global state
                let mut state = state_clone.lock().unwrap();
                state.active_workers -= 1;
                state.current_cpu_usage -= task.kind.cpu_cost();
            }
        });
        worker_handles.push(handle);
    }

    println!("Spawned 8 worker threads.");

    // The Monitor Thread
    let simulation_running = Arc::new(AtomicBool::new(true));
    let monitor_running_flag = Arc::clone(&simulation_running);
    let monitor_state = Arc::clone(&shared_state);

    let monitor_handle = thread::spawn(move || {
        println!("Monitor thread started.");

        // Vectors to store our 10ms interval data
        let mut cpu_records = Vec::new();
        let mut worker_records = Vec::new();

        while monitor_running_flag.load(Ordering::Relaxed) {
            let state = monitor_state.lock().unwrap();

            cpu_records.push(state.current_cpu_usage);
            worker_records.push(state.active_workers);

            // uncommenting will spam the terminal
            //println!("Monitor [10ms]: {} workers active, {}% CPU used", state.active_workers, state.current_cpu_usage);

            drop(state);
            thread::sleep(Duration::from_millis(10));
        }
        println!("Monitor thread shutting down.");

        // Return the recorded data back to the main thread
        (cpu_records, worker_records)
    });

    // The Manager Dispatcher Loop (FIFO)
    println!("Manager starting dispatch loop...");

    // Start the timer for the "makespan" (total runtime) metric
    let start_time = Instant::now();
    let total_tasks = manager_queue.len();

    while let Some(task) = manager_queue.pop_front() {
        loop {
            let mut state = shared_state.lock().unwrap();

            if state.can_dispatch(&task) {
                state.active_workers += 1;
                state.current_cpu_usage += task.kind.cpu_cost();

                tx.send(task).unwrap();
                break;
            } else {
                drop(state);
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    // --- CLEAN SHUTDOWN & METRICS EXPORT ---
    //Drop the sender so workers know to stop once they finish their current tasks
    drop(tx);

    // Wait for workers to finish all dispatched tasks
        for handle in worker_handles {
        handle.join().unwrap();
    }

    // Stop the timer now that all work is actually done
    let makespan = start_time.elapsed();
    println!("Manager finished dispatching and workers completed all tasks.");

    // Tell the monitor to stop and collect its data
    simulation_running.store(false, Ordering::Relaxed);
    let (cpu_history, worker_history) = monitor_handle.join().unwrap();

    // Calculate the averages requested by the amendments
    let avg_cpu: f64 = cpu_history.iter().copied().map(|x| x as f64).sum::<f64>() / cpu_history.len() as f64;
    let avg_workers: f64 = worker_history.iter().copied().map(|x| x as f64).sum::<f64>() / worker_history.len() as f64;

    // Write the experiment output to a text file for the GitHub repo
    let mut file = File::create("fifo_metrics.txt").expect("Unable to create metrics file");
    writeln!(file, "--- FIFO EXPERIMENT METRICS ---").unwrap();
    writeln!(file, "Total Tasks Completed: {}", total_tasks).unwrap();
    writeln!(file, "Makespan (Total Runtime): {:.2?}", makespan).unwrap();
    writeln!(file, "Average CPU Usage: {:.2}%", avg_cpu).unwrap();
    writeln!(file, "Average Active Workers: {:.2} out of 8", avg_workers).unwrap();
    writeln!(file, "Total Monitor Ticks (10ms intervals): {}", cpu_history.len()).unwrap();

    println!("Simulation complete. Metrics successfully written to 'fifo_metrics.txt'.");
}