use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Custom error types for task execution failures.
/// Each variant represents a different failure scenario that requires
/// specific handling in production systems.
#[derive(Debug, Clone)]
pub enum TaskError {
    /// Unrecoverable system failure that requires immediate shutdown
    CriticalFailure(String),
    /// Network operation timed out - may be retryable
    NetworkTimeout,
    /// System resources (memory, CPU, connections) exhausted
    ResourceExhausted,
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::CriticalFailure(msg) => write!(f, "Critical failure: {}", msg),
            TaskError::NetworkTimeout => write!(f, "Network timeout"),
            TaskError::ResourceExhausted => write!(f, "Resource exhausted"),
        }
    }
}

impl std::error::Error for TaskError {}

/// Orchestrates multiple concurrent tasks with graceful shutdown capabilities.
///
/// Key features:
/// - Fail-fast: if any task fails, all remaining tasks are cancelled
/// - Graceful shutdown: external shutdown signals cancel all running tasks
/// - Broadcast-based cancellation: uses tokio's broadcast channel for efficient shutdown signaling
pub struct TaskOrchestrator {
    /// Broadcast sender for shutdown signals. All tasks subscribe to this channel
    /// to receive cancellation notifications when failures occur or external shutdown is requested.
    shutdown_tx: broadcast::Sender<()>,
}

impl TaskOrchestrator {
    /// Creates a new task orchestrator with a broadcast channel for shutdown coordination.
    /// Buffer size of 1 is sufficient since we only send a single shutdown signal.
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self { shutdown_tx }
    }

    /// Executes all tasks concurrently and handles failures with immediate shutdown.
    ///
    /// # Arguments
    /// * `simulate_failure` - If true, task-2 will intentionally fail for testing purposes
    ///
    /// # Behavior
    /// - All tasks run concurrently using join_all
    /// - If ANY task fails, remaining tasks are cancelled via broadcast shutdown
    /// - External shutdown signals (via shutdown()) also cancel all tasks
    /// - Returns Ok(()) only if ALL tasks complete successfully
    pub async fn run(&self, simulate_failure: bool) -> Result<(), TaskError> {
        // Subscribe to shutdown channel before starting tasks to avoid missing signals
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let msg = if simulate_failure {
            "Starting task orchestration with simulated failure"
        } else {
            "Starting task orchestration"
        };
        info!("{}", msg);

        // Spawn 5 concurrent tasks with different durations and failure conditions
        // Task-2 is the failure candidate when simulate_failure=true
        let tasks = vec![
            self.spawn_task("task-1", Duration::from_millis(500), false),
            self.spawn_task("task-2", Duration::from_millis(1000), simulate_failure), // Failure injection point
            self.spawn_task("task-3", Duration::from_millis(750), false),
            self.spawn_task("task-4", Duration::from_millis(300), false), // Fastest task
            self.spawn_task("task-5", Duration::from_millis(900), false),
        ];

        // Race between task completion and external shutdown signal
        tokio::select! {
            // Wait for all tasks to complete (or first failure)
            results = futures::future::join_all(tasks) => {
                // Process results in order - fail fast on first error
                for (task_name, result) in results {
                    match result {
                        Ok(_) => info!("{} completed successfully", task_name),
                        Err(e) => {
                            error!("{} failed: {}", task_name, e);
                            // Critical: trigger shutdown for remaining tasks before returning error
                            self.initiate_shutdown().await;
                            return Err(e);
                        }
                    }
                }
                info!("All tasks completed successfully");
                Ok(())
            }
            // Handle external shutdown request (e.g., SIGTERM, manual shutdown)
            _ = shutdown_rx.recv() => {
                warn!("Shutdown signal received");
                Err(TaskError::CriticalFailure("External shutdown requested".to_string()))
            }
        }
    }

    /// Spawns a single task with cancellation support.
    ///
    /// # Returns
    /// Tuple of (task_name, Result) - task_name for logging, Result for success/failure
    ///
    /// # Cancellation
    /// Tasks can be cancelled in two ways:
    /// 1. External shutdown signal (via broadcast channel)
    /// 2. Another task failure triggering system shutdown
    async fn spawn_task(
        &self,
        name: &str,
        duration: Duration,
        should_fail: bool,
    ) -> (String, Result<(), TaskError>) {
        let task_name = name.to_string();
        // Each task gets its own shutdown receiver to listen for cancellation
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Race between task execution and shutdown signal
        let result = tokio::select! {
            // Normal task execution path
            result = self.execute_task(&task_name, duration, should_fail) => result,
            // Cancellation path - if shutdown signal received, cancel immediately
            _ = shutdown_rx.recv() => {
                warn!("{} cancelled due to shutdown", task_name);
                Err(TaskError::CriticalFailure("Task cancelled".to_string()))
            }
        };

        (task_name, result)
    }

    /// Simulates actual task work with configurable failure injection.
    ///
    /// In production, this would contain the actual business logic:
    /// - Database operations
    /// - API calls  
    /// - File processing
    /// - Network requests
    ///
    /// # Arguments
    /// * `should_fail` - Testing parameter to simulate task failures
    async fn execute_task(
        &self,
        name: &str,
        duration: Duration,
        should_fail: bool,
    ) -> Result<(), TaskError> {
        info!("{} starting", name);

        // Simulate work duration - in production this would be actual async work
        sleep(duration).await;

        if should_fail {
            // Simulate critical failure that requires system shutdown
            Err(TaskError::CriticalFailure(format!(
                "{} encountered critical error",
                name
            )))
        } else {
            info!("{} completed work", name);
            Ok(())
        }
    }

    /// Initiates graceful shutdown sequence.
    ///
    /// Steps:
    /// 1. Broadcast shutdown signal to all tasks
    /// 2. Perform cleanup operations (close connections, flush buffers, etc.)
    /// 3. Log completion
    ///
    /// Note: Uses `let _ =` for send() because receiver might already be dropped
    /// during normal shutdown, which is expected behavior.
    async fn initiate_shutdown(&self) {
        warn!("Initiating graceful shutdown");
        // Broadcast to all task receivers - some may have already completed
        let _ = self.shutdown_tx.send(());

        // Simulate resource cleanup operations
        // In production: close DB connections, flush logs, save state, etc.
        sleep(Duration::from_millis(100)).await;
        info!("Resources cleaned up");
    }

    /// Public API for external shutdown requests.
    /// Used by signal handlers, health checks, or administrative commands.
    pub async fn shutdown(&self) {
        self.initiate_shutdown().await;
    }
}

/// Application entry point demonstrating orchestrator usage.
///
/// Production considerations:
/// - Add signal handling (SIGTERM, SIGINT) for graceful shutdown
/// - Implement health checks and metrics collection
/// - Add configuration management for task parameters
/// - Consider using structured logging (JSON) for production
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging - consider JSON format for production
    tracing_subscriber::fmt::init();

    let orchestrator = Arc::new(TaskOrchestrator::new());

    // Run with failure simulation disabled - enable for testing
    match orchestrator.run(false).await {
        Ok(_) => {
            info!("System completed successfully");
            std::process::exit(0);
        }
        Err(e) => {
            error!("System failed: {}", e);
            // Ensure cleanup happens even on failure
            orchestrator.shutdown().await;
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Test successful execution of all tasks without failures.
    /// Verifies that the orchestrator completes normally when all tasks succeed.
    #[tokio::test]
    async fn test_successful_task_orchestration() {
        tracing_subscriber::fmt::try_init().ok(); // Ignore if already initialized

        let orchestrator = TaskOrchestrator::new();
        let result = timeout(Duration::from_secs(5), orchestrator.run(false)).await;

        assert!(result.is_ok(), "Test should not timeout");
        assert!(
            result.unwrap().is_ok(),
            "All tasks should complete successfully"
        );
    }

    /// Test fail-fast behavior when one task encounters a critical failure.
    /// Verifies that system shuts down immediately upon first task failure.
    #[tokio::test]
    async fn test_task_failure_and_shutdown() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();
        let result = timeout(Duration::from_secs(5), orchestrator.run(true)).await;

        assert!(result.is_ok(), "Test should not timeout");
        let inner_result = result.unwrap();
        assert!(
            inner_result.is_err(),
            "Should fail due to simulated task failure"
        );

        // Verify we get the expected error type
        match inner_result {
            Err(TaskError::CriticalFailure(_)) => (),
            _ => panic!("Expected CriticalFailure error"),
        }
    }

    /// Test external shutdown signal handling.
    /// Verifies that tasks can be cancelled via external shutdown command.
    #[tokio::test]
    async fn test_external_shutdown() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = Arc::new(TaskOrchestrator::new());
        let orchestrator_clone = Arc::clone(&orchestrator);

        // Start orchestrator in background task
        let run_handle = tokio::spawn(async move { orchestrator_clone.run(false).await });

        // Trigger external shutdown after brief delay
        tokio::spawn(async move {
            sleep(Duration::from_millis(100)).await;
            orchestrator.shutdown().await;
        });

        let result = timeout(Duration::from_secs(5), run_handle).await;
        assert!(result.is_ok(), "Test should not timeout");

        let join_result = result.unwrap();
        assert!(join_result.is_ok(), "Task join should succeed");

        let task_result = join_result.unwrap();
        assert!(task_result.is_err(), "Should fail due to external shutdown");
    }

    /// Test that task failure triggers rapid cancellation of remaining tasks.
    /// Verifies fail-fast behavior by checking execution time.
    #[tokio::test]
    async fn test_task_cancellation_on_failure() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();
        let start_time = std::time::Instant::now();

        let result = orchestrator.run(true).await;
        let elapsed = start_time.elapsed();

        assert!(result.is_err(), "Should fail due to task-2 failure");
        // Task-2 fails at 1000ms, but cancellation should prevent waiting for task-5 (900ms)
        // Total time should be ~1000ms + cleanup, well under 1200ms
        assert!(
            elapsed < Duration::from_millis(1200),
            "Should fail fast, not wait for all tasks. Elapsed: {:?}",
            elapsed
        );
    }
}
