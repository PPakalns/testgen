use crate::dos2unix;
use crate::task_units::{Contest, GlobalConfig, Task};
use anyhow::Result;
use futures::future::join_all;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::task::JoinHandle;
use walkdir::WalkDir;

pub async fn compile_validator(validator: &Path, output: &Path) -> anyhow::Result<()> {
    println!("Compiling validator {}", validator.display());
    let status = Command::new("g++")
        .arg("-Wall")
        .arg("-std=c++17")
        .arg("-o")
        .arg(output)
        .arg(validator)
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("g++ failed: {}", status);
    }
    Ok(())
}

pub async fn extract_tests(
    test_zip: &Path,
    target_dir: &Path,
    gctx: &Arc<GlobalConfig>,
) -> anyhow::Result<()> {
    if target_dir.exists() {
        tokio::fs::remove_dir_all(target_dir).await?;
    }
    tokio::fs::create_dir_all(target_dir).await?;
    println!(
        "Extracting '{}' to '{}'",
        test_zip.display(),
        target_dir.display()
    );
    let status = Command::new("unzip")
        .arg(test_zip)
        .arg("-d")
        .arg(target_dir)
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("unzip failed: {}", status);
    }
    if gctx.dos2unix {
        println!(
            "Applying dos2unix to all files in '{}'",
            target_dir.display()
        );

        let mut to_complete: Vec<JoinHandle<anyhow::Result<()>>> = vec![];

        for entry in WalkDir::new(target_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
        {
            let permit = gctx.plimit.clone().acquire_owned().await?;
            let path = entry.path().to_path_buf();

            to_complete.push(tokio::spawn(async move {
                // dos2unix::dos2unix_command(path);
                dos2unix::dos2unix_async(path).await?;
                drop(permit);
                Ok(())
            }));
        }
        for f in to_complete.into_iter() {
            f.await??;
        }
    }
    Ok(())
}

pub struct TaskValidationResult {
    pub state: String,
    pub task: Task,
    pub tests: Option<Tests>,
    pub test_assignment: Option<TestAssignment>,
    pub exception: Option<String>,
}

impl TaskValidationResult {
    pub fn new(task: Task) -> TaskValidationResult {
        TaskValidationResult {
            state: "unresolved".into(),
            task,
            tests: None,
            test_assignment: None,
            exception: None,
        }
    }

    pub fn set_tests(&mut self, tests: Tests) {
        self.tests = Some(tests);
    }
    pub fn set_test_assignment(&mut self, ta: TestAssignment) {
        self.test_assignment = Some(ta);
    }
    pub fn set_success(&mut self) {
        if self.state == "unresolved" {
            self.state = "success".into();
        }
    }
    pub fn set_fail(&mut self, e: String) {
        self.state = "fail".into();
        self.exception = Some(e);
    }

    pub fn print_summary(&self) {
        use colored::Colorize;
        println!("{}", "========================================".blue());
        self.task.print_summary();
        if let Some(ta) = &self.test_assignment {
            ta.tests.print_summary();
            ta.print_summary();
        } else if let Some(tests) = &self.tests {
            tests.print_summary();
        }
        if self.success() {
            println!("{}", "GREAT SUCCESS".bold().green());
        } else if self.failed() {
            eprintln!("{}", "VALIDATION FAILED".bold().red());
            if let Some(e) = &self.exception {
                eprintln!("{}", e.red());
            }
        } else {
            println!("{}", "VALIDATION NOT FINISHED".yellow());
        }
        println!("{}", "========================================".blue());
    }

    pub fn success(&self) -> bool {
        !self.failed() && self.state == "success"
    }
    pub fn failed(&self) -> bool {
        self.state == "fail"
    }
}

pub struct ContestValidationResult {
    pub contest: Contest,
    pub task_validation_results: Vec<TaskValidationResult>,
}

impl ContestValidationResult {
    pub fn print_summary(&self) {
        use colored::Colorize;
        println!("\n");
        let header = format!("### Contest Summary: {} ###", self.contest.name)
            .bold()
            .magenta();
        println!("{}", header);
        self.contest.print_summary();
        for tr in &self.task_validation_results {
            tr.print_summary();
        }
    }
}

pub enum ValidationResult {
    Task(TaskValidationResult),
    Contest(ContestValidationResult),
}

impl ValidationResult {
    pub fn print_summary(&self) {
        match self {
            ValidationResult::Task(t) => t.print_summary(),
            ValidationResult::Contest(c) => c.print_summary(),
        }
    }
}

pub async fn validate_task(task: &Task, gctx: Arc<GlobalConfig>) -> TaskValidationResult {
    let mut result = TaskValidationResult::new(task.clone());
    let test_dir = PathBuf::from("testi_validator").join(&task.name);

    let assignment_res: Result<TestAssignment, anyhow::Error> = async {
        let compiled_validator =
            PathBuf::from("testi_validator").join(format!("validator{}", task.name));

        let tests = Tests::new(
            task.point_config.clone(),
            &task.point_file,
            &test_dir,
            task.public_groups.clone(),
        );

        let compile = compile_validator(&task.validator, &compiled_validator);

        let extract = async {
            if !gctx.extract {
                return Ok(());
            }
            extract_tests(&task.test_archive, &test_dir, &gctx).await
        };

        // Compile validator and execute everything else in parallel
        let (mut tests, (), ()) = tokio::try_join!(tests, compile, extract,)?;

        let subtasks_vec: Vec<usize> = (0..task.subtask_points.len()).collect();
        tests
            .match_subtasks(&compiled_validator, &subtasks_vec, &gctx)
            .await;
        let assignment = TestAssignment::new(task.subtask_points.clone(), tests);

        assignment.tests.print_summary();
        assignment.print_summary();
        assignment.validate()?;
        Ok::<TestAssignment, anyhow::Error>(assignment)
    }
    .await;

    match assignment_res {
        Ok(assignment) => {
            result.set_test_assignment(assignment);
            result.set_success();
        }
        Err(e) => {
            result.set_fail(format!("{}", e));
        }
    }

    result
}

pub struct Test {
    pub tid: String,
    pub file: PathBuf,
}

impl Test {
    pub fn new(tid: String, file: PathBuf) -> Self {
        Self { tid, file }
    }

    pub async fn validate(&self, validator: &Path, subtask: usize) -> bool {
        // run validator --group <subtask> with stdin from file
        let mut cmd = Command::new(validator);
        cmd.arg("--group").arg(subtask.to_string());
        // open sync file for stdin so it can be converted into Stdio
        if let Ok(f) = std::fs::File::open(&self.file) {
            cmd.stdin(std::process::Stdio::from(f));
        }
        let status = cmd.status().await;
        match status {
            Ok(s) if s.success() => {
                println!("\t{} : {:3}  OK!", self.file.display(), subtask);
                true
            }
            _ => {
                println!("\t{} : {:3}", self.file.display(), subtask);
                false
            }
        }
    }
}

pub struct TestGroup {
    pub gid: usize,
    pub points: i32,
    pub tests: HashMap<String, Test>,
    pub subtask_matches: HashSet<usize>,
}

impl TestGroup {
    pub fn new(gid: usize, points: i32) -> Self {
        Self {
            gid,
            points,
            tests: HashMap::new(),
            subtask_matches: HashSet::new(),
        }
    }

    pub fn set_tests(&mut self, files: HashMap<String, PathBuf>) {
        self.tests.clear();
        for (k, v) in files.into_iter() {
            let key = k.clone();
            self.tests.insert(key, Test::new(k, v));
        }
    }
    pub async fn match_subtasks(
        &mut self,
        validator: &Path,
        subtask_list: &[usize],
        gctx: &Arc<GlobalConfig>,
    ) {
        if self.tests.is_empty() {
            panic!("No tests available");
        }
        self.subtask_matches.clear();
        // spawn a job for every (test, subtask) pair and limit concurrency with the provided global semaphore
        let mut handles = Vec::new();
        for &subtask in subtask_list {
            for test in self.tests.values() {
                let gctx = gctx.clone();
                let test_path = test.file.clone();
                let validator = validator.to_path_buf();
                let permit = gctx.plimit.clone().acquire_owned().await.unwrap();

                handles.push(tokio::spawn(async move {
                    let t = Test::new(String::new(), test_path.clone());
                    let r = t.validate(&validator, subtask).await;
                    drop(permit);
                    (subtask, r)
                }));
            }
        }
        let results = join_all(handles).await;
        // collect results per subtask
        let mut subtask_results: HashMap<usize, Vec<bool>> = HashMap::new();
        for r in results {
            match r {
                Ok((s, ok)) => {
                    subtask_results.entry(s).or_default().push(ok);
                }
                Err(_) => { /* treat as failure by leaving missing entries or false */ }
            }
        }
        for &subtask in subtask_list {
            let mut add_match = true;
            let res_vec = subtask_results.get(&subtask);
            let test_count = self.tests.values().len();
            if let Some(vs) = res_vec {
                if vs.len() != test_count {
                    add_match = false;
                } else {
                    for ok in vs {
                        if !*ok {
                            add_match = false;
                            break;
                        }
                    }
                }
            } else {
                add_match = false;
            }
            if add_match {
                self.subtask_matches.insert(subtask);
            }
        }
    }
}

pub struct Tests {
    pub public_groups: Vec<usize>,
    pub groups: BTreeMap<usize, TestGroup>,
}

impl Tests {
    pub async fn new(
        point_config: Option<serde_yaml::Value>,
        point_file: &Path,
        test_dir: &Path,
        public_groups: Vec<usize>,
    ) -> Result<Tests> {
        let mut groups = BTreeMap::new();
        let mut pubs = public_groups;
        if let Some(pc) = point_config {
            // parse provided structure
            let (pts, public) = parse_points(pc)?;
            pubs = public;
            for (gid, pts) in pts {
                groups.insert(gid, TestGroup::new(gid, pts));
            }
        } else {
            let pts = read_points(point_file).await?;
            for (gid, pts) in pts {
                groups.insert(gid as usize, TestGroup::new(gid as usize, pts));
            }
        }
        let input_files = get_input_files(test_dir).await?;
        for (gid, files) in input_files {
            if !groups.contains_key(&gid) {
                anyhow::bail!("{} group missing from point file!", gid);
            }
            groups.get_mut(&gid).unwrap().set_tests(files);
        }
        Ok(Tests {
            public_groups: pubs,
            groups,
        })
    }

    pub async fn match_subtasks(
        &mut self,
        validator: &Path,
        subtask_list: &[usize],
        gctx: &Arc<GlobalConfig>,
    ) {
        for tg in self.groups.values_mut() {
            tg.match_subtasks(validator, subtask_list, gctx).await;
        }
    }

    pub fn print_summary(&self) {
        use colored::Colorize;
        let mut total_public_points = 0i32;
        for pgid in &self.public_groups {
            if let Some(g) = self.groups.get(pgid) {
                total_public_points += g.points;
            }
        }
        let mut total_test_count = 0usize;
        for group in self.groups.values() {
            total_test_count += group.tests.len();
        }
        println!(
            "\t{} {}",
            "Test group cnt:".yellow().bold(),
            self.groups.len()
        );
        println!(
            "\t{} {}",
            "Total test cnt:".yellow().bold(),
            total_test_count
        );
        println!(
            "\t{} {}",
            "Total public points:".yellow().bold(),
            total_public_points
        );
    }
}

pub struct TestAssignment {
    pub subtask_points: Vec<i32>,
    pub tests: Tests,
    pub assigned_groups: BTreeMap<usize, Option<usize>>,
}

impl TestAssignment {
    pub fn new(subtask_points: Vec<i32>, tests: Tests) -> TestAssignment {
        let assigned_groups = tests.groups.keys().map(|&gid| (gid, None)).collect();
        let mut ta = TestAssignment {
            subtask_points,
            tests,
            assigned_groups,
        };
        ta.assign_groups();
        ta
    }

    fn assign_groups(&mut self) {
        for subtask_id in 0..self.subtask_points.len() {
            println!("Processing subtask {}", subtask_id);
            let mut points_needed = self.subtask_points[subtask_id];
            let mut gids: Vec<usize> = self.assigned_groups.keys().cloned().collect();
            gids.sort();
            for gid in gids {
                if let Some(Some(_)) = self.assigned_groups.get(&gid) {
                    continue;
                }
                let tg = self.tests.groups.get(&gid).unwrap();
                if !tg.subtask_matches.contains(&subtask_id) {
                    continue;
                }
                if tg.points > points_needed {
                    continue;
                }
                points_needed -= tg.points;
                self.assigned_groups.insert(gid, Some(subtask_id));
            }
        }
    }

    pub fn get_summary(&self) -> (i32, Vec<usize>, Vec<i32>, Vec<HashSet<usize>>, i32) {
        let mut points_assigned = 0i32;
        let mut unused_groups: HashSet<usize> = HashSet::new();
        let mut subtask_assigned_points = vec![0i32; self.subtask_points.len()];
        let mut subtask_group_assignment: Vec<HashSet<usize>> =
            vec![HashSet::new(); self.subtask_points.len()];
        for (&gid, &opt) in &self.assigned_groups {
            match opt {
                None => {
                    unused_groups.insert(gid);
                }
                Some(subtask_id) => {
                    let group = self.tests.groups.get(&gid).unwrap();
                    subtask_assigned_points[subtask_id] += group.points;
                    subtask_group_assignment[subtask_id].insert(gid);
                    points_assigned += group.points;
                }
            }
        }
        let total_points: i32 = self.subtask_points.iter().sum();
        (
            points_assigned,
            unused_groups.into_iter().collect(),
            subtask_assigned_points,
            subtask_group_assignment,
            total_points,
        )
    }

    pub fn validate(&self) -> Result<()> {
        let (
            points_assigned,
            unused_groups,
            subtask_assigned_points,
            subtask_group_assignment,
            total_points,
        ) = self.get_summary();
        let mut errors: Vec<String> = Vec::new();
        if points_assigned != 100 {
            errors.push(format!("Bad assignment {}/100;", points_assigned));
        }
        if !unused_groups.is_empty() {
            errors.push(format!("Unused groups {:?};", unused_groups));
        }
        if subtask_assigned_points != self.subtask_points {
            errors.push(format!(
                "Incorrectly assigned points Expected: {:?}, Got: {:?};",
                self.subtask_points, subtask_assigned_points
            ));
        }
        if total_points != 100 {
            errors.push(format!("Total points {} != 100;", total_points));
        }
        if subtask_group_assignment.iter().any(|s| s.is_empty()) {
            errors.push("Subtask without any assigned group;".to_string());
        }
        if !errors.is_empty() {
            errors.insert(0, "ASSIGNMENT FAIL:".to_string());
            anyhow::bail!(errors.join("\n"));
        }
        Ok(())
    }

    pub fn print_summary(&self) {
        use colored::Colorize;
        let (
            points_assigned,
            unused_groups,
            subtask_assigned_points,
            subtask_group_assignment,
            total_points,
        ) = self.get_summary();
        println!();
        println!("\t{} {:?}", "Unused groups:".yellow().bold(), unused_groups);
        println!(
            "\t{} {:?}",
            "Expected points:".yellow().bold(),
            self.subtask_points
        );
        println!(
            "\t{} {:?}",
            "Points per subtask:".yellow().bold(),
            subtask_assigned_points
        );
        println!("\n");

        let group_count = self.tests.groups.len();
        // Build colored Pub.gr. row
        let mut pubgr_cells: Vec<String> = vec![" ".to_string(); group_count];
        for &pg in &self.tests.public_groups {
            if pg < group_count {
                pubgr_cells[pg] = "X".cyan().bold().to_string();
            }
        }
        println!("{:7} {}", "Pub.gr.".yellow().bold(), pubgr_cells.join(""));

        // Build colored Ap.uzd. row (digits)
        let apuzd_cells: Vec<String> = (0..group_count)
            .map(|x| {
                char::from_digit((x % 10) as u32, 10)
                    .unwrap()
                    .to_string()
                    .bright_white()
                    .to_string()
            })
            .collect();
        println!("{:7} {}", "Ap.uzd.".yellow().bold(), apuzd_cells.join(""));

        for subtask_id in 0..self.subtask_points.len() {
            // initialize cells: mark public groups with '│' so they show even when not matching
            let mut gr_cells: Vec<String> = vec![" ".to_string(); group_count];
            for &public_group in &self.tests.public_groups {
                if public_group < group_count {
                    gr_cells[public_group] = "│".cyan().to_string();
                }
            }
            // groups that match this subtask: use the same match char '░', color cyan if group is public
            for group in self.tests.groups.values() {
                if group.subtask_matches.contains(&subtask_id) {
                    if group.gid < group_count {
                        if self.tests.public_groups.contains(&group.gid) {
                            gr_cells[group.gid] = "░".bright_cyan().to_string();
                        } else {
                            gr_cells[group.gid] = "░".yellow().to_string();
                        }
                    }
                }
            }
            // assigned groups - override with strong color
            for &gid in &subtask_group_assignment[subtask_id] {
                if gid < group_count {
                    // green bold block for assigned groups
                    gr_cells[gid] = "▓".green().bold().to_string();
                }
            }
            let line = gr_cells.join("");
            println!("{:7} {}", subtask_id, line);
        }

        if points_assigned == total_points {
            println!(
                "{} {} / {}",
                "Points:".bold(),
                points_assigned.to_string().green(),
                total_points.to_string().green()
            );
        } else {
            println!(
                "{} {} / {}",
                "Points:".bold(),
                points_assigned.to_string().red(),
                total_points.to_string().red()
            );
        }
    }
}

pub async fn get_input_files(
    test_folder: &Path,
) -> Result<HashMap<usize, HashMap<String, PathBuf>>> {
    let mut test_files: HashMap<usize, HashMap<String, PathBuf>> = HashMap::new();
    let matcher = regex::Regex::new(r"\.(i|o)(\d+)([a-z]*)$").unwrap();
    let mut read_dir = tokio::fs::read_dir(test_folder).await?;
    loop {
        let Some(entry) = read_dir.next_entry().await? else {
            break;
        };

        let file = entry.path();
        if file.is_dir() {
            anyhow::bail!("Unexpected directory {}", file.display());
        }
        let file_name = file.file_name().unwrap().to_string_lossy();
        if let Some(caps) = matcher.captures(&file_name) {
            if &caps[1] == "o" {
                continue;
            }
            let group: usize = caps[2].parse()?;
            let key = caps
                .get(3)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            test_files.entry(group).or_default();
            let group_map = test_files.get_mut(&group).unwrap();
            if group_map.contains_key(&key) {
                anyhow::bail!("Duplicated test group {}", file.display());
            }
            group_map.insert(key, file);
        } else {
            anyhow::bail!("File doesn't match with regex {}", file.display());
        }
    }
    Ok(test_files)
}

pub async fn read_points(point_file: &Path) -> Result<HashMap<i32, i32>> {
    println!("Reading point file {}", point_file.display());
    let content = tokio::fs::read_to_string(point_file).await?;
    let mut points_per_group: HashMap<i32, i32> = HashMap::new();
    let mut max_group = 0i32;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let vars: Vec<String> = line
            .replace("-", " ")
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let a: i32 = vars[0].parse()?;
        let b: i32 = vars[1].parse()?;
        let points: i32 = vars[2].parse()?;
        for g in a..=b {
            if points_per_group.contains_key(&g) {
                anyhow::bail!("Duplicated groups in point file");
            }
            points_per_group.insert(g, points);
        }
        if b > max_group {
            max_group = b;
        }
    }
    points_per_group.entry(0).or_insert(0);
    for i in 0..=max_group {
        if !points_per_group.contains_key(&i) {
            anyhow::bail!("Point file is not continious. Check group {}", i);
        }
    }
    Ok(points_per_group)
}

pub fn parse_points(value: serde_yaml::Value) -> Result<(HashMap<usize, i32>, Vec<usize>)> {
    // This function handles a simplified subset: accept a YAML sequence of mappings
    let mut points_per_group: HashMap<usize, i32> = HashMap::new();
    let mut public_groups: HashSet<usize> = HashSet::new();
    if let serde_yaml::Value::Sequence(seq) = value {
        for v in seq {
            if let serde_yaml::Value::Mapping(map) = v {
                let groups_key = serde_yaml::Value::String("groups".into());
                let points_key = serde_yaml::Value::String("points".into());
                let groups_v = map
                    .get(&groups_key)
                    .ok_or_else(|| anyhow::anyhow!("Missing groups"))?;
                let points_v = map
                    .get(&points_key)
                    .ok_or_else(|| anyhow::anyhow!("Missing points"))?;
                let groups: Vec<usize> = if let serde_yaml::Value::Number(n) = groups_v {
                    vec![n.as_i64().unwrap() as usize]
                } else if let serde_yaml::Value::Sequence(s2) = groups_v {
                    // interval
                    let from = s2[0].as_i64().unwrap() as usize;
                    let to = s2[1].as_i64().unwrap() as usize;
                    (from..=to).collect()
                } else {
                    anyhow::bail!("Unparsabled groups")
                };
                let points = points_v
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("Provided points are not points"))?
                    as i32;
                for g in &groups {
                    if points_per_group.contains_key(g) {
                        anyhow::bail!("Duplicated groups in point file");
                    }
                    points_per_group.insert(*g, points);
                }
                let public_key = serde_yaml::Value::String("public".into());
                if let Some(pub_v) = map.get(&public_key) {
                    match pub_v {
                        serde_yaml::Value::Bool(true) => {
                            for g in groups.iter() {
                                public_groups.insert(*g);
                            }
                        }
                        serde_yaml::Value::Sequence(seqpub) => {
                            for pv in seqpub {
                                let idx = pv.as_i64().unwrap() as usize;
                                public_groups.insert(idx);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // ensure contiguous groups
    for i in 0..points_per_group.len() {
        if !points_per_group.contains_key(&i) {
            anyhow::bail!("Missing group from point file");
        }
    }
    let sum: i32 = points_per_group.values().sum();
    if sum != 100 {
        anyhow::bail!("Points for all groups doesn't sum up to 100");
    }
    Ok((
        points_per_group.into_iter().collect(),
        public_groups.into_iter().collect(),
    ))
}
