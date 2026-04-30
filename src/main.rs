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

// Add this right below your imports and above the TaskKind enum
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub total_tasks: u32,
    pub io_probability: f64,
    pub use_optimized_scheduler: bool,
    pub output_filename: &'static str,
}

// The core task structure
#[derive(Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub kind: TaskKind,
    pub arrival_time: Duration,
    pub duration: Duration, // Will always be 200ms based on amendments
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
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
            started_at: None,
            completed_at: None,
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

fn run_simulation(config: SimulationConfig) {
    println!("Concurrent Task Dispatcher initialized.");

    // Generate and Sort Tasks
    let mut generated_tasks = generate_tasks(config.total_tasks, config.io_probability);
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

    // Channel for workers to send completed tasks back
    let (done_tx, done_rx) = mpsc::channel::<Task>();

    // Spawn exactly 8 worker threads
    let mut worker_handles = vec![];

    for _ in 0..8 {
        let rx_clone = Arc::clone(&shared_rx);
        let state_clone = Arc::clone(&shared_state);
        let done_tx_clone = done_tx.clone();

        let handle = thread::spawn(move || {
            loop {
                // Safely grab the next task from the channel
                let mut task = {
                    let lock = rx_clone.lock().unwrap();
                    match lock.recv() {
                        Ok(t) => t,
                        Err(_) => break, // If the channel closes, the thread shuts down cleanly
                    }
                };

                // Stamp start time, execute, then stamp completion time
                // Simulate executing the task
                task.started_at = Some(Instant::now());
                thread::sleep(task.duration);
                task.completed_at = Some(Instant::now());

                // Task is done! Release the resources back to the global state
                let mut state = state_clone.lock().unwrap();
                state.active_workers -= 1;
                state.current_cpu_usage -= task.kind.cpu_cost();
                drop(state);

                // Send the finished task to the collector
                done_tx_clone.send(task).unwrap();
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

    // The Manager Dispatcher Loop (FIFO & Optimized)
    println!("Manager starting dispatch loop...");
    let start_time = Instant::now();

    while !manager_queue.is_empty() {
        let mut state = shared_state.lock().unwrap();
        let now = start_time.elapsed();

        if config.use_optimized_scheduler {
            // OPTIMIZED: Find the first task in line that has arrived AND fits the resources
            let mut found_index = None;
            for (i, task) in manager_queue.iter().enumerate() {
                if task.arrival_time > now {
                    break; // Hasn't arrived yet
                }
                if state.can_dispatch(task) {
                    found_index = Some(i);
                    break;
                }
            }

            if let Some(index) = found_index {
                let task = manager_queue.remove(index).unwrap();
                state.active_workers += 1;
                state.current_cpu_usage += task.kind.cpu_cost();
                tx.send(task).unwrap();
            } else {
                drop(state);
                thread::sleep(Duration::from_millis(5));
            }
        } else {
            // FIFO: Only look at the front of the queue
            if let Some(task) = manager_queue.front() {
                if task.arrival_time <= now {
                    if state.can_dispatch(task) {
                        let task = manager_queue.pop_front().unwrap();
                        state.active_workers += 1;
                        state.current_cpu_usage += task.kind.cpu_cost();
                        tx.send(task).unwrap();
                    } else {
                        drop(state);
                        thread::sleep(Duration::from_millis(5)); // Resources full
                    }
                } else {
                    drop(state);
                    thread::sleep(Duration::from_millis(5)); // Task hasn't arrived
                }
            }
        }
    }

    // --- CLEAN SHUTDOWN & METRICS EXPORT ---
    //Drop the sender so workers know to stop once they finish their current tasks
    drop(tx);
    drop(done_tx); // Drop the main thread's copy of the done sender

    // Wait for workers to finish all dispatched tasks
        for handle in worker_handles {
        handle.join().unwrap();
    }

    // Stop the timer now that all work is actually done
    let makespan = start_time.elapsed();
    //println!("Manager finished dispatching and workers completed all tasks.");

    // Tell the monitor to stop and collect its data
    simulation_running.store(false, Ordering::Relaxed);
    let (cpu_history, worker_history) = monitor_handle.join().unwrap();

    // Tally up the wait and turnaround times
    let mut total_wait_time = Duration::ZERO;
    let mut total_turnaround_time = Duration::ZERO;
    let mut completed_count: u32 = 0;

    // Pull all finished tasks out of the collector channel
    for task in done_rx {
        completed_count += 1;
        let logical_arrival = start_time + task.arrival_time;
        
        if let (Some(started), Some(completed)) = (task.started_at, task.completed_at) {
            total_wait_time += started.duration_since(logical_arrival);
            total_turnaround_time += completed.duration_since(logical_arrival);
        }
    }

    // Calculate the averages requested by the amendments
    let avg_wait = total_wait_time / completed_count;
    let avg_turnaround = total_turnaround_time / completed_count;
    let avg_cpu: f64 = cpu_history.iter().copied().map(|x| x as f64).sum::<f64>() / cpu_history.len() as f64;
    let avg_workers: f64 = worker_history.iter().copied().map(|x| x as f64).sum::<f64>() / worker_history.len() as f64;

// Write the experiment output to a text file for the GitHub repo
    let mut file = File::create(config.output_filename).expect("Unable to create metrics file");
    writeln!(file, "--- EXPERIMENT METRICS ---").unwrap();
    writeln!(file, "Policy: {}", if config.use_optimized_scheduler { "Optimized" } else { "FIFO" }).unwrap();
    writeln!(file, "Total Tasks Completed: {}", completed_count).unwrap();
    writeln!(file, "Makespan (Total Runtime): {:.2?}", makespan).unwrap();
    writeln!(file, "Average Wait Time: {:.2?}", avg_wait).unwrap();
    writeln!(file, "Average Turnaround Time: {:.2?}", avg_turnaround).unwrap();
    writeln!(file, "Average CPU Usage: {:.2}%", avg_cpu).unwrap();
    writeln!(file, "Average Active Workers: {:.2} out of 8", avg_workers).unwrap();

    println!("Simulation complete. Metrics successfully written to '{}'.", config.output_filename);
}

fn main() {
    // Experiment A: Strict FIFO Workload
    let fifo_config = SimulationConfig {
        total_tasks: 1000,
        io_probability: 0.70, // 700 IO / 300 CPU
        use_optimized_scheduler: false,
        output_filename: "fifo_metrics.txt",
    };
    run_simulation(fifo_config);

    // Experiment B: Stressed Workload using Optimized Search
    let optimized_config = SimulationConfig {
        total_tasks: 1000,
        io_probability: 0.80, // 800 IO / 200 CPU
        use_optimized_scheduler: true,
        output_filename: "optimized_metrics.txt",
    };
    run_simulation(optimized_config);
    
    println!("Both experiments completed successfully!");
}