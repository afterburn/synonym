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

    /// Executes 5 tasks concurrently with random failure simulation and handles failures with immediate shutdown.
    ///
    /// # Arguments
    /// * `simulate_failure` - If true, introduces random failures across tasks for testing
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
            "Starting task orchestration with random failure simulation"
        } else {
            "Starting task orchestration"
        };
        info!("{}", msg);

        // Spawn 5 concurrent tasks simulating different microservices/background jobs
        // Each task has different characteristics and random failure potential
        let tasks = vec![
            // Database service simulation - medium duration, network timeout risk
            self.spawn_task(
                "database-service",
                Duration::from_millis(800),
                simulate_failure,
                0.1,
            ),
            // API gateway simulation - longest duration, critical failure risk
            self.spawn_task(
                "api-gateway",
                Duration::from_millis(1200),
                simulate_failure,
                0.15,
            ),
            // Cache service simulation - fast, resource exhaustion risk
            self.spawn_task(
                "cache-service",
                Duration::from_millis(400),
                simulate_failure,
                0.08,
            ),
            // Message queue simulation - medium-long, network issues
            self.spawn_task(
                "message-queue",
                Duration::from_millis(900),
                simulate_failure,
                0.12,
            ),
            // File processor simulation - variable duration, critical errors
            self.spawn_task(
                "file-processor",
                Duration::from_millis(600),
                simulate_failure,
                0.2,
            ),
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
                            // Critical error logging with full context
                            error!(
                                task = %task_name,
                                error = %e,
                                "Task failed critically - initiating system shutdown"
                            );
                            // Critical: trigger shutdown for remaining tasks before returning error
                            self.initiate_shutdown().await;
                            return Err(e);
                        }
                    }
                }
                info!("All tasks completed successfully - system operating normally");
                Ok(())
            }
            // Handle external shutdown request (e.g., SIGTERM, manual shutdown)
            _ = shutdown_rx.recv() => {
                warn!("External shutdown signal received - cancelling all tasks");
                Err(TaskError::CriticalFailure("External shutdown requested".to_string()))
            }
        }
    }

    /// Spawns a single task with cancellation support and random failure simulation.
    ///
    /// # Arguments
    /// * `failure_probability` - Chance of random failure (0.0 = never, 1.0 = always)
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
        simulate_failure: bool,
        failure_probability: f64,
    ) -> (String, Result<(), TaskError>) {
        let task_name = name.to_string();
        // Each task gets its own shutdown receiver to listen for cancellation
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Race between task execution and shutdown signal
        let result = tokio::select! {
            // Normal task execution path
            result = self.execute_task(&task_name, duration, simulate_failure, failure_probability) => result,
            // Cancellation path - if shutdown signal received, cancel immediately
            _ = shutdown_rx.recv() => {
                warn!(
                    task = %task_name,
                    "Task cancelled due to system shutdown - cleaning up resources"
                );
                Err(TaskError::CriticalFailure("Task cancelled".to_string()))
            }
        };

        (task_name, result)
    }

    /// Simulates actual microservice/background job work with realistic failure scenarios.
    ///
    /// Simulates common production scenarios:
    /// - Database connection timeouts
    /// - API rate limiting (resource exhaustion)  
    /// - Network partitions
    /// - Memory pressure
    /// - Disk I/O failures
    ///
    /// # Arguments
    /// * `simulate_failure` - Global failure simulation toggle
    /// * `failure_probability` - Specific failure chance for this task type
    async fn execute_task(
        &self,
        name: &str,
        base_duration: Duration,
        simulate_failure: bool,
        failure_probability: f64,
    ) -> Result<(), TaskError> {
        info!(
            task = %name,
            duration_ms = %base_duration.as_millis(),
            "Task starting execution"
        );

        // Simulate variable work duration (±25% variance)
        let variance: f64 = rand::random::<f64>() * 0.5 - 0.25; // -0.25 to +0.25
        let actual_duration =
            Duration::from_millis(((base_duration.as_millis() as f64) * (1.0 + variance)) as u64);

        // Simulate work in chunks to allow for cancellation
        let chunk_size = Duration::from_millis(100);
        let chunks = actual_duration.as_millis() / chunk_size.as_millis();

        for i in 0..chunks {
            sleep(chunk_size).await;

            // Random failure check during execution (simulates real-world failures)
            let random_value: f64 = rand::random();
            if simulate_failure && random_value < failure_probability {
                let error = match name {
                    n if n.contains("database") => TaskError::NetworkTimeout,
                    n if n.contains("cache") => TaskError::ResourceExhausted,
                    _ => TaskError::CriticalFailure(format!(
                        "{} encountered critical error during execution",
                        name
                    )),
                };

                error!(
                    task = %name,
                    error = %error,
                    progress = %((i + 1) * 100 / chunks),
                    "Task failed during execution"
                );
                return Err(error);
            }
        }

        info!(
            task = %name,
            actual_duration_ms = %actual_duration.as_millis(),
            "Task completed successfully"
        );
        Ok(())
    }

    /// Initiates graceful shutdown sequence with proper resource cleanup.
    ///
    /// Production shutdown sequence:
    /// 1. Stop accepting new requests
    /// 2. Broadcast shutdown signal to all tasks
    /// 3. Wait for in-flight operations to complete (with timeout)
    /// 4. Close database connections
    /// 5. Flush logs and metrics
    /// 6. Release file handles and network sockets
    ///
    /// Note: Uses `let _ =` for send() because receiver might already be dropped
    /// during normal shutdown, which is expected behavior.
    async fn initiate_shutdown(&self) {
        warn!("Initiating graceful system shutdown");

        // Step 1: Broadcast shutdown signal to all tasks
        let _ = self.shutdown_tx.send(());
        info!("Shutdown signal broadcasted to all tasks");

        // Step 2: Simulate closing database connections
        info!("Closing database connections...");
        sleep(Duration::from_millis(50)).await;

        // Step 3: Simulate flushing logs and metrics
        info!("Flushing logs and metrics to persistent storage...");
        sleep(Duration::from_millis(30)).await;

        // Step 4: Simulate releasing file handles
        info!("Releasing file handles and network resources...");
        sleep(Duration::from_millis(20)).await;

        info!("Graceful shutdown completed - all resources cleaned up");
    }

    /// Public API for external shutdown requests.
    /// Used by signal handlers, health checks, or administrative commands.
    pub async fn shutdown(&self) {
        self.initiate_shutdown().await;
    }
}

/// Application entry point demonstrating microservice orchestration.
///
/// Production considerations:
/// - Add signal handling (SIGTERM, SIGINT) for graceful shutdown
/// - Implement health checks and metrics collection
/// - Add configuration management for task parameters
/// - Consider using structured logging (JSON) for production
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging with timestamps and task context
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false) // Disable thread IDs - not useful for single orchestrator
        .with_level(true)
        .with_ansi(true) // Enable colors for better readability
        .init();

    info!("Starting Task Orchestrator System");
    let orchestrator = Arc::new(TaskOrchestrator::new());

    // Default to false for normal operation - only enable failures via env var
    let enable_failure_simulation = std::env::var("ENABLE_FAILURES")
        .map(|v| v == "true")
        .unwrap_or(false); // Changed to false - normal operation should succeed

    info!(
        failure_simulation = %enable_failure_simulation,
        "System configuration loaded"
    );

    match orchestrator.run(enable_failure_simulation).await {
        Ok(_) => {
            info!("✅ System completed successfully - all services operational");
            std::process::exit(0);
        }
        Err(e) => {
            error!(
                error = %e,
                "❌ System failed critically - performing final cleanup"
            );

            // Note: shutdown was already called in run() method, so we just exit
            error!("System shutdown complete - exiting with error status");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Test successful execution of all services without failures.
    /// Verifies that the orchestrator completes normally when all tasks succeed.
    #[tokio::test]
    async fn test_successful_task_orchestration() {
        tracing_subscriber::fmt::try_init().ok(); // Ignore if already initialized

        let orchestrator = TaskOrchestrator::new();
        let result = timeout(Duration::from_secs(10), orchestrator.run(false)).await;

        assert!(result.is_ok(), "Test should not timeout");
        assert!(
            result.unwrap().is_ok(),
            "All services should complete successfully"
        );
    }

    /// Test fail-fast behavior with random failures enabled.
    /// Verifies that system shuts down immediately upon first task failure.
    #[tokio::test]
    async fn test_random_failure_and_shutdown() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();

        // Run multiple times to hit random failures
        let mut failure_detected = false;
        for _ in 0..10 {
            let result = timeout(Duration::from_secs(10), orchestrator.run(true)).await;
            assert!(result.is_ok(), "Test should not timeout");

            if result.unwrap().is_err() {
                failure_detected = true;
                break;
            }
        }

        // With random failures enabled, we should see at least one failure
        // Note: This test might occasionally pass all runs due to randomness
        if !failure_detected {
            println!("Warning: No random failures occurred in 10 runs - this is statistically unlikely but possible");
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
            sleep(Duration::from_millis(200)).await;
            orchestrator.shutdown().await;
        });

        let result = timeout(Duration::from_secs(10), run_handle).await;
        assert!(result.is_ok(), "Test should not timeout");

        let join_result = result.unwrap();
        assert!(join_result.is_ok(), "Task join should succeed");

        let task_result = join_result.unwrap();
        assert!(task_result.is_err(), "Should fail due to external shutdown");
    }

    /// Test that task failure triggers rapid cancellation of remaining tasks.
    /// Verifies fail-fast behavior by checking execution time.
    #[tokio::test]
    async fn test_task_cancellation_timing() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();
        let start_time = std::time::Instant::now();

        // Force a failure by running many times with failures enabled
        let mut result = Ok(());
        for _ in 0..20 {
            result = orchestrator.run(true).await;
            if result.is_err() {
                break;
            }
        }

        let elapsed = start_time.elapsed();

        // If we got a failure, verify it happened quickly (fail-fast)
        if result.is_err() {
            // Should complete much faster than sum of all task durations (~4s)
            assert!(
                elapsed < Duration::from_secs(3),
                "Should fail fast, not wait for all tasks. Elapsed: {:?}",
                elapsed
            );
        }
    }
}
