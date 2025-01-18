//! # Async Example
//! 
//! This example demonstrates thread-safe concurrent access to a Commodore disk drive
//! using rs1541's mutex-protected OpenCBM handle. It spawns two concurrent tasks:
//! 
//! * Task 1 identifies the drive and reads its directory
//! * Task 2 polls the drive status multiple times at fixed intervals
//! 
//! The example shows:
//! * Using Arc to share the CBM handle between tasks
//! * Safe concurrent access to drive operations
//! * Mixing different operations (directory, status, etc) concurrently
//! * Proper error handling with async/await
//!
//! Note that while concurrent access is safe, the 1541 drive itself processes
//! commands sequentially. The example demonstrates the safety of concurrent
//! access rather than parallel execution of drive operations.
//!
//! To run:
//! ```bash
//! cargo run --example async
//! ```use rs1541::Cbm;

use rs1541::Cbm;
use std::error::Error;
use tokio;
use std::sync::Arc;
use std::fmt;

// Create a wrapper error type
#[derive(Debug)]
struct TaskError(String);

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Task error: {}", self.0)
    }
}

impl Error for TaskError {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cbm = Arc::new(Cbm::new()?);
    
    // Thread 1
    let cbm1 = Arc::clone(&cbm);
    let task1 = tokio::spawn(async move {
        let id = cbm1.identify(8)
            .map_err(|e| TaskError(e.to_string()))?;
        println!("Task 1 - Drive type at device 8: {}", id);
        
        let dir = cbm1.dir(8, None)
            .map_err(|e| TaskError(e.to_string()))?;
        println!("Task 1 - Directory listing:\n{}", dir);
        
        Ok::<(), TaskError>(())
    });

    // Thread 2
    let cbm2 = Arc::clone(&cbm);
    let task2 = tokio::spawn(async move {
        for i in 1..=3 {
            let status = cbm2.get_status(8)
                .map_err(|e| TaskError(e.to_string()))?;
            println!("Task 2 (iteration {}) - Drive status: {}", i, status);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        Ok::<(), TaskError>(())
    });

    // Wait for both threads to complete
    let (result1, result2) = tokio::join!(task1, task2);
    result1.unwrap().map_err(|e| Box::new(e) as Box<dyn Error>)?;
    result2.unwrap().map_err(|e| Box::new(e) as Box<dyn Error>)?;

    Ok(())
}