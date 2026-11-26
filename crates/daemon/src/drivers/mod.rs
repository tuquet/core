pub mod agent;
pub mod automation;
pub mod browser;
pub mod cdp;
pub mod http;
pub mod shell;

use crate::core::masker::SecretMasker;
use crate::protocol::schema::{DriverType, Job, JobId, JobResult, LogChannel, LogChunk};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Context passed to each driver during job execution
#[derive(Clone)]
pub struct ExecutionContext {
    pub job_id: JobId,
    pub log_sender: UnboundedSender<LogChunk>,
    pub masker: Arc<SecretMasker>,
    pub cancel_token: CancellationToken,
}

impl ExecutionContext {
    pub fn new(
        job_id: impl Into<JobId>,
        log_sender: UnboundedSender<LogChunk>,
        masker: Arc<SecretMasker>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            log_sender,
            masker,
            cancel_token,
        }
    }

    /// Emits a sanitized log chunk to the transport stream
    pub fn emit_log(&self, channel: LogChannel, raw_text: &str) {
        let masked_text = self.masker.mask(raw_text);
        let chunk = LogChunk::new(&self.job_id, channel, masked_text);
        let _ = self.log_sender.send(chunk);
    }
}

/// Common trait that all pluggable execution drivers must implement
#[async_trait]
pub trait ExecutionDriver: Send + Sync {
    /// Identifier for this driver (e.g. "shell", "agent", "http")
    fn name(&self) -> &'static str;

    /// Checks if this driver can execute the given job
    fn can_handle(&self, job: &Job) -> bool;

    /// Executes the job and returns the final JobResult
    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String>;

    /// Requests cancellation of a running job.
    /// By default, drivers rely on cooperative cancellation via `ctx.cancel_token`.
    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        Ok(())
    }
}

/// Registry holding active drivers
pub struct DriverRegistry {
    drivers: BTreeMap<String, Box<dyn ExecutionDriver>>,
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, driver: Box<dyn ExecutionDriver>) {
        self.drivers.insert(driver.name().to_string(), driver);
    }

    pub fn get(&self, name: &str) -> Option<&dyn ExecutionDriver> {
        self.drivers.get(name).map(|b| b.as_ref())
    }

    pub fn find_for_job(&self, job: &Job) -> Option<&dyn ExecutionDriver> {
        if let Some(driver) = self
            .drivers
            .values()
            .map(|b| b.as_ref())
            .find(|d| d.can_handle(job))
        {
            return Some(driver);
        }
        match &job.driver {
            DriverType::Custom(name) => self.get(name),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::schema::Job;

    struct CustomEchoDriver;

    #[async_trait]
    impl ExecutionDriver for CustomEchoDriver {
        fn name(&self) -> &'static str {
            "custom-echo"
        }
        fn can_handle(&self, job: &Job) -> bool {
            matches!(&job.driver, DriverType::Custom(n) if n == "echo-alias")
        }
        async fn execute(&self, job: Job, _ctx: ExecutionContext) -> Result<JobResult, String> {
            Ok(JobResult::success(job.id, 0, None))
        }
    }

    #[test]
    fn find_for_job_respects_can_handle_and_custom_name() {
        let mut registry = DriverRegistry::new();
        registry.register(Box::new(CustomEchoDriver));

        let aliased = Job {
            id: "j1".to_string(),
            driver: DriverType::Custom("echo-alias".to_string()),
            payload: serde_json::json!({}),
            cwd: None,
            env: None,
            timeout_ms: 1000,
            created_at: None,
        };
        assert_eq!(
            registry.find_for_job(&aliased).map(|d| d.name()),
            Some("custom-echo")
        );

        let by_name = Job {
            driver: DriverType::Custom("custom-echo".to_string()),
            ..aliased
        };
        assert_eq!(
            registry.find_for_job(&by_name).map(|d| d.name()),
            Some("custom-echo")
        );
    }
}
