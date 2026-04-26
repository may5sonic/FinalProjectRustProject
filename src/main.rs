use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;

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

fn main() {
    println!("Concurrent Task Dispatcher initialized.");

    // Test Layer 2 with the first experiment's 700/300 split (70% IO)
    let experiment_1_tasks = generate_tasks(1000, 0.70);

    // Quick verification to ensure our randomizer is working
    let io_count = experiment_1_tasks.iter().filter(|t| matches!(t.kind, TaskKind::IO)).count();
    let cpu_count = experiment_1_tasks.len() - io_count;

    println!(
        "Generated {} tasks: {} IO, {} CPU",
        experiment_1_tasks.len(),
        io_count,
        cpu_count
    );
}