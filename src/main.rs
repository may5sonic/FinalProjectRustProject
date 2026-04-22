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

fn main() {
    println!("Concurrent Task Dispatcher initialized.");
}