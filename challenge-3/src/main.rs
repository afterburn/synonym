use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub enum TaskError {
    CriticalFailure(String),
    NetworkTimeout,
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

pub struct TaskOrchestrator {
    shutdown_tx: broadcast::Sender<()>,
}

impl TaskOrchestrator {
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self { shutdown_tx }
    }

    pub async fn run(&self) -> Result<(), TaskError> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        info!("Starting task orchestration");

        let tasks = vec![
            self.spawn_task("task-1", Duration::from_millis(500), false),
            self.spawn_task("task-2", Duration::from_millis(1000), false),
            self.spawn_task("task-3", Duration::from_millis(750), false),
            self.spawn_task("task-4", Duration::from_millis(300), false),
            self.spawn_task("task-5", Duration::from_millis(900), false),
        ];

        tokio::select! {
            results = futures::future::join_all(tasks) => {
                for (task_name, result) in results {
                    match result {
                        Ok(_) => info!("{} completed successfully", task_name),
                        Err(e) => {
                            error!("{} failed: {}", task_name, e);
                            self.initiate_shutdown().await;
                            return Err(e);
                        }
                    }
                }
                info!("All tasks completed successfully");
                Ok(())
            }
            _ = shutdown_rx.recv() => {
                warn!("Shutdown signal received");
                Err(TaskError::CriticalFailure("External shutdown requested".to_string()))
            }
        }
    }

    pub async fn run_with_failure(&self) -> Result<(), TaskError> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        info!("Starting task orchestration with simulated failure");

        let tasks = vec![
            self.spawn_task("task-1", Duration::from_millis(500), false),
            self.spawn_task("task-2", Duration::from_millis(1000), true), // This will fail
            self.spawn_task("task-3", Duration::from_millis(750), false),
            self.spawn_task("task-4", Duration::from_millis(300), false),
            self.spawn_task("task-5", Duration::from_millis(900), false),
        ];

        tokio::select! {
            results = futures::future::join_all(tasks) => {
                for (task_name, result) in results {
                    match result {
                        Ok(_) => info!("{} completed successfully", task_name),
                        Err(e) => {
                            error!("{} failed: {}", task_name, e);
                            self.initiate_shutdown().await;
                            return Err(e);
                        }
                    }
                }
                info!("All tasks completed successfully");
                Ok(())
            }
            _ = shutdown_rx.recv() => {
                warn!("Shutdown signal received");
                Err(TaskError::CriticalFailure("External shutdown requested".to_string()))
            }
        }
    }

    async fn spawn_task(
        &self,
        name: &str,
        duration: Duration,
        should_fail: bool,
    ) -> (String, Result<(), TaskError>) {
        let task_name = name.to_string();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let result = tokio::select! {
            result = self.execute_task(&task_name, duration, should_fail) => result,
            _ = shutdown_rx.recv() => {
                warn!("{} cancelled due to shutdown", task_name);
                Err(TaskError::CriticalFailure("Task cancelled".to_string()))
            }
        };

        (task_name, result)
    }

    async fn execute_task(
        &self,
        name: &str,
        duration: Duration,
        should_fail: bool,
    ) -> Result<(), TaskError> {
        info!("{} starting", name);

        sleep(duration).await;

        if should_fail {
            Err(TaskError::CriticalFailure(format!(
                "{} encountered critical error",
                name
            )))
        } else {
            info!("{} completed work", name);
            Ok(())
        }
    }

    async fn initiate_shutdown(&self) {
        warn!("Initiating graceful shutdown");
        let _ = self.shutdown_tx.send(());

        // Simulate resource cleanup
        sleep(Duration::from_millis(100)).await;
        info!("Resources cleaned up");
    }

    pub async fn shutdown(&self) {
        self.initiate_shutdown().await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let orchestrator = Arc::new(TaskOrchestrator::new());

    match orchestrator.run().await {
        Ok(_) => {
            info!("System completed successfully");
            std::process::exit(0);
        }
        Err(e) => {
            error!("System failed: {}", e);
            orchestrator.shutdown().await;
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_successful_task_orchestration() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();
        let result = timeout(Duration::from_secs(5), orchestrator.run()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_task_failure_and_shutdown() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();
        let result = timeout(Duration::from_secs(5), orchestrator.run_with_failure()).await;

        assert!(result.is_ok());
        let inner_result = result.unwrap();
        assert!(inner_result.is_err());

        match inner_result {
            Err(TaskError::CriticalFailure(_)) => (),
            _ => panic!("Expected CriticalFailure error"),
        }
    }

    #[tokio::test]
    async fn test_external_shutdown() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = Arc::new(TaskOrchestrator::new());
        let orchestrator_clone = Arc::clone(&orchestrator);

        let run_handle = tokio::spawn(async move { orchestrator_clone.run().await });

        // Trigger shutdown after a short delay
        tokio::spawn(async move {
            sleep(Duration::from_millis(100)).await;
            orchestrator.shutdown().await;
        });

        let result = timeout(Duration::from_secs(5), run_handle).await;
        assert!(result.is_ok());

        let join_result = result.unwrap();
        assert!(join_result.is_ok());

        let task_result = join_result.unwrap();
        assert!(task_result.is_err());
    }

    #[tokio::test]
    async fn test_task_cancellation_on_failure() {
        tracing_subscriber::fmt::try_init().ok();

        let orchestrator = TaskOrchestrator::new();
        let start_time = std::time::Instant::now();

        let result = orchestrator.run_with_failure().await;
        let elapsed = start_time.elapsed();

        assert!(result.is_err());
        // Should fail before all tasks complete (faster than 1000ms)
        assert!(elapsed < Duration::from_millis(1200));
    }
}
