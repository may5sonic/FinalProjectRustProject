# FinalProjectRustProject

# Concurrent Task Dispatcher

## Build and Run Instructions
This project is built using standard Cargo structure. There are no external dependencies required other than the `rand` crate for task generation. 

To compile the project, open your terminal in the root directory and run:
`cargo build`

To run the simulation, execute:
`cargo run`

Because the simulation accurately models task arrival times over a 20-second period for two separate workloads, the full execution will take approximately 45-50 seconds. Once finished, the program automatically writes the metrics to `fifo_metrics.txt` and `optimized_metrics.txt` in the root directory.

## Example Commands
Since the simulation is pre-configured to run both the strict FIFO workload and the Optimized workload automatically, no complex CLI arguments are necessary. 

* Check syntax and warnings: `cargo check`
* Run the full dual-simulation: `cargo run`

## Summary of Design
This system implements a central dispatcher architecture using Rust's concurrency primitives. 
* **State Management:** A global `SystemState` struct is wrapped in an `Arc<Mutex<SystemState>>` to safely track active workers (max 8) and global CPU usage (max 100%) across all threads.
* **Concurrency:** The system utilizes 10 active threads: 1 Main Manager thread, 8 Worker threads, and 1 Monitor thread.
* **Task Distribution:** The Manager pulls tasks from a standard `VecDeque` and dispatches them to the Worker pool using an `mpsc` channel (`tx`/`rx`). 
* **Completion Tracking:** Workers simulate execution using `thread::sleep`, stamp the completion time, and send the task back to the main thread via a second `done_tx` channel so wait times and turnaround times can be accurately calculated.

## Summary of Experiments
* **Experiment A (Balanced/FIFO):** Processes 1,000 tasks with a 70% IO / 30% CPU split. The Manager uses strict FIFO logic, causing Head-of-Line blocking if a CPU task reaches the front of the queue but sufficient CPU resources are not available.
* **Experiment B (Stressed/Optimized):** Processes 1,000 tasks with an 80% IO / 20% CPU split. The Manager scans the queue to dispatch the first task that fits the current system resources (Shortest-Queue / Fit policy), significantly reducing makespan and improving CPU utilization.

## Tool Use Disclosure
* **Tools Used:** Gemini AI
* **Kind of Help Provided:** Architecture planning, Rust syntax correction, and troubleshooting concurrency type-casting errors.
* **Advice Accepted:** I accepted the advice to use a secondary `mpsc` channel (`done_tx`) for workers to return completed tasks back to the main thread. This avoided complex ownership issues and allowed the main thread to safely calculate final wait and turnaround timestamps.
* **Advice Rejected / Fixed:** I initially considered keeping the Monitor thread printing its 10ms intervals directly to the terminal, but I rejected this approach as it created too much console spam. Instead, I collected the metrics into vectors and calculated the averages at the end of the simulation for a cleaner output.