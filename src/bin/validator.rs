use clap::Parser;
use num_cpus;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use testgen::task_units::{load_contest, load_task, load_yaml_with_base, GlobalConfig};
use testgen::validator::{validate_task, ValidationResult};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long)]
    dos2unix: bool,
    #[clap(long = "use-extracted")]
    extract_true: Option<bool>,
    #[clap(required = true)]
    config: Vec<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let gctx = Arc::new(GlobalConfig {
        dos2unix: args.dos2unix,
        extract: args.extract_true.unwrap_or(true),
        plimit: Arc::new(Semaphore::new(num_cpus::get())),
    });

    // For each provided config, spawn a task that validates it (task or contest).
    let mut handles: Vec<JoinHandle<anyhow::Result<ValidationResult>>> = Vec::new();

    for cfg in args.config.iter() {
        let config_path = PathBuf::from(cfg);
        let gctx = gctx.clone();
        handles.push(tokio::spawn(async move {
            // Load YAML to determine if it's a contest or a single task
            let config = load_yaml_with_base(&config_path).await?;

            if config.get("tasks").is_some() {
                read_and_validate_contest(&config_path, config, gctx).await
            } else {
                read_and_validate_task(&config_path, config, gctx).await
            }
        }));
    }

    // Collect all validation results; print final summaries after each config completes
    for h in handles {
        match h.await? {
            Ok(vr) => vr.print_summary(),
            Err(e) => eprintln!("Validation failed: {}", e),
        }
    }

    Ok(())
}

async fn read_and_validate_contest(
    path: &Path,
    doc: serde_yaml::Value,
    gctx: Arc<GlobalConfig>,
) -> anyhow::Result<ValidationResult> {
    // contest
    let contest = load_contest(&path, doc).await?;
    // validate all tasks concurrently, printing immediate per-task outputs inside validate_task
    let mut task_handles = Vec::new();

    for task in contest.tasks.iter().cloned() {
        let gctx = gctx.clone();
        task_handles.push(tokio::spawn(
            async move { validate_task(&task, gctx).await },
        ));
    }
    let mut results = Vec::new();
    for th in task_handles {
        let tr = th.await.unwrap();
        // immediate per-task summary already printed inside validate_task
        results.push(tr);
    }
    let cv = ValidationResult::Contest(testgen::validator::ContestValidationResult {
        contest,
        task_validation_results: results,
    });
    Ok(cv)
}

async fn read_and_validate_task(
    path: &Path,
    doc: serde_yaml::Value,
    gctx: Arc<GlobalConfig>,
) -> anyhow::Result<ValidationResult> {
    let task = load_task(&path, doc).await?;
    let tr = validate_task(&task, gctx).await;
    // tr already printed immediate summaries
    Ok(ValidationResult::Task(tr))
}
