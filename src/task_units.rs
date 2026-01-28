use anyhow::Result;
use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct GlobalConfig {
    // dos2unix
    pub dos2unix: bool,
    // Extract
    pub extract: bool,
    // Global semaphore shared across all validations
    pub plimit: Arc<Semaphore>,
}

pub async fn load_yaml_with_base(config_path: &Path) -> Result<Value> {
    let mut last_path = config_path.to_path_buf();

    let yaml_str = tokio::fs::read_to_string(&config_path).await?;
    let mut config: Value = serde_yaml::from_str(&yaml_str)?;

    while let Value::Mapping(ref mut m) = config {
        let base_key = Value::String("base".into());

        let Some(base_val) = m.remove(&base_key) else {
            break;
        };

        let base_path = match base_val {
            Value::String(s) => last_path.parent().unwrap_or_else(|| Path::new(".")).join(s),
            _ => anyhow::bail!("base must be a string"),
        };
        last_path = base_path;
        let base_config: Value = serde_yaml::from_str(&fs::read_to_string(&last_path)?)?;

        config = merge_yaml(base_config, Value::Mapping(m.clone()));
    }
    Ok(config)
}

fn merge_yaml(base: Value, override_v: Value) -> Value {
    use serde_yaml::Value::*;
    match (base, override_v) {
        (Mapping(mut a), Mapping(b)) => {
            for (k, v) in b.into_iter() {
                if a.contains_key(&k) {
                    let aval = a.remove(&k).unwrap();
                    a.insert(k, merge_yaml(aval, v));
                } else {
                    a.insert(k, v);
                }
            }
            Mapping(a)
        }
        (_b, o) => o,
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub name: String,
    pub title: String,
    pub public_groups: Vec<usize>,
    pub test_archive: PathBuf,
    pub validator: PathBuf,
    pub point_file: PathBuf,
    pub point_config: Option<Value>,
    pub subtask_points: Vec<i32>,
}

impl Task {
    pub fn print_summary(&self) {
        use colored::Colorize;
        let header = format!("=== Task: {}: {} ===", self.name, self.title)
            .bold()
            .cyan();
        println!("{}", header);
        println!(
            "\t{} {:?}",
            "Public groups:".yellow().bold(),
            self.public_groups
        );
    }
}

#[derive(Clone, Debug)]
pub struct Contest {
    pub name: String,
    pub description: String,
    pub tasks: Vec<Arc<Task>>,
}

impl Contest {
    pub fn print_summary(&self) {
        use colored::Colorize;
        let header = format!("*** Contest: {} - {} ***", self.name, self.description)
            .bold()
            .magenta();
        println!("\n\n{}", header);
        println!(
            "\t{} {:?}",
            "Task summary:".yellow().bold(),
            self.tasks
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
        );
    }
}

pub async fn load_task(config_path: &Path, config: serde_yaml::Value) -> Result<Task> {
    let task_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let title = config
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let public_groups = config
        .get("public_groups")
        .and_then(|v| v.as_sequence())
        .map(|s| {
            s.iter()
                .filter_map(|x| x.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or(vec![0, 1]);
    let test_archive = task_dir.join(
        config
            .get("tests_archive")
            .and_then(|v| v.as_str())
            .unwrap_or(
                config
                    .get("test_archive")
                    .and_then(|v| v.as_str())
                    .unwrap_or("testi.zip"),
            ),
    );
    let validator = task_dir.join(
        config
            .get("validator")
            .and_then(|v| v.as_str())
            .unwrap_or("riki/validator.cpp"),
    );
    let point_file = task_dir.join(
        config
            .get("point_file")
            .and_then(|v| v.as_str())
            .unwrap_or("punkti.txt"),
    );
    let point_config = config.get("tests_groups").cloned();
    let subtask_points = config
        .get("subtask_points")
        .and_then(|v| v.as_sequence())
        .map(|s| {
            s.iter()
                .filter_map(|x| x.as_i64().map(|n| n as i32))
                .collect()
        })
        .unwrap_or(vec![0, 2]);
    Ok(Task {
        name,
        title,
        public_groups,
        test_archive,
        validator,
        point_file,
        point_config,
        subtask_points,
    })
}

pub async fn load_contest(config_path: &Path, config: serde_yaml::Value) -> Result<Contest> {
    let contest_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let description = config
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut tasks = Vec::new();
    if let Some(tasks_map) = config.get("tasks") {
        if let Value::Mapping(map) = tasks_map {
            for (k, v) in map {
                let task_name = k.as_str().unwrap_or("");
                let cfg = v
                    .get("config")
                    .and_then(|x| x.as_str())
                    .map(|s| contest_dir.join(s))
                    .unwrap_or_else(|| contest_dir.join(task_name).join("task.yaml"));

                let doc = load_yaml_with_base(&cfg).await?;
                let task = load_task(&cfg, doc).await?;
                tasks.push(Arc::new(task));
            }
        }
    }
    Ok(Contest {
        name,
        description,
        tasks,
    })
}
