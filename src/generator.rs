use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;
use zip::write::FileOptions;

pub fn compile(source: &Path, output: &Path) -> Result<()> {
    println!("Compiling {} to {}", source.display(), output.display());
    let status = Command::new("g++")
        .arg("-Wall")
        .arg("-std=c++14")
        .arg("-g")
        .arg("-o")
        .arg(output)
        .arg(source)
        .status()?;
    if !status.success() {
        anyhow::bail!("g++ failed: {}", status);
    }
    Ok(())
}

#[derive(Debug)]
pub struct TestGen {
    pub filename: String,
    pub output_dir: PathBuf,
    _temp_dir: tempfile::TempDir,
    generator: PathBuf,
    solution: PathBuf,
    boi: bool,
    test_group: i32,
    test_in_group: i32,
    pub group_list: Vec<(i32, String)>,
    pub test_list: Vec<(i32, i32)>,
}

impl TestGen {
    pub fn new(
        filename: &str,
        generator_src: &Path,
        solution_src: &Path,
        output_dir: &Path,
        boi: bool,
    ) -> Result<TestGen> {
        if output_dir.exists() {
            fs::remove_dir_all(output_dir)?;
        }
        fs::create_dir_all(output_dir)?;

        let temp_dir = tempdir()?;
        let generator = temp_dir.path().join("generator");
        let solution = temp_dir.path().join("solution");
        compile(generator_src, &generator)?;
        compile(solution_src, &solution)?;

        Ok(TestGen {
            filename: filename.to_string(),
            output_dir: output_dir.to_path_buf(),
            _temp_dir: temp_dir,
            generator,
            solution,
            boi,
            test_group: -1,
            test_in_group: 0,
            group_list: Vec::new(),
            test_list: Vec::new(),
        })
    }

    pub fn end(self) -> Result<()> {
        println!("Summary:");
        let mut cnt = -1i32;
        let mut points = 0i32;
        for ginfo in &self.group_list {
            cnt += 1;
            points += ginfo.0;
            println!("\tGroup {:02}: {} {}\t{}", cnt, ginfo.0, points, ginfo.1);
        }
        println!("TOTAL POINTS: {}", points);
        if !self.boi {
            assert_eq!(points, 100);
        }
        // temp dir cleaned up when dropped
        Ok(())
    }

    pub fn new_group(&mut self, points: i32, comment: Option<&str>, public: bool) {
        let mut comment_str = comment.map(|s| s.to_string()).unwrap_or_else(|| {
            if let Some(prev) = self.group_list.last() {
                prev.1.clone()
            } else {
                String::new()
            }
        });
        self.test_group += 1;
        self.test_in_group = 0;
        if public {
            comment_str.push_str(" --- PUBLISKA GRUPA");
        }
        self.group_list.push((points, comment_str));
        println!("\nGroup {}\n", self.test_group);
    }

    pub fn increase_test(&mut self) {
        self.test_in_group += 1;
    }

    pub fn get_extension(&self, input: bool, test_id: Option<(i32, i32)>) -> String {
        let test_id = test_id.unwrap_or((self.test_group, self.test_in_group));
        let io_letter = if input { 'i' } else { 'o' };
        if self.boi {
            assert_eq!(test_id.1, 0);
            return format!(".{}{:02}", io_letter, test_id.0);
        }
        let letter = ((test_id.1 as u8) + b'a') as char;
        format!(".{}{:02}{}", io_letter, test_id.0, letter)
    }

    pub fn get_input_file(&self, test_id: Option<(i32, i32)>) -> PathBuf {
        self.output_dir.join(format!(
            "{}{}",
            self.filename,
            self.get_extension(true, test_id)
        ))
    }

    pub fn get_output_file(&self, test_id: Option<(i32, i32)>) -> PathBuf {
        self.output_dir.join(format!(
            "{}{}",
            self.filename,
            self.get_extension(false, test_id)
        ))
    }

    pub fn generate_answer(&self, input: &Path, output: &Path) -> Result<()> {
        println!("Generating answer {}", output.display());
        let fin = std::fs::File::open(input)?;
        let fout = std::fs::File::create(output)?;
        let status = Command::new(&self.solution)
            .stdin(fin)
            .stdout(fout)
            .status()?;
        if !status.success() {
            anyhow::bail!("solution failed: {}", status);
        }
        Ok(())
    }

    pub fn store_test(&mut self) {
        self.test_list.push((self.test_group, self.test_in_group));
    }

    pub fn generate_test<I: AsRef<str>>(&mut self, args: &[I]) -> Result<()> {
        self.store_test();
        let args: Vec<String> = args.iter().map(|s| s.as_ref().to_string()).collect();
        let input = self.get_input_file(None);
        println!("Generating test {} , args: {:?}", input.display(), args);
        let fin = std::fs::File::create(&input)?;
        let status = Command::new(&self.generator)
            .args(&args)
            .stdout(std::process::Stdio::from(fin.try_clone()?))
            .status()?;
        if !status.success() {
            anyhow::bail!("generator failed: {}", status);
        }
        self.generate_answer(&input, &self.get_output_file(None))?;
        self.increase_test();
        Ok(())
    }

    pub fn generate_raw_test(&mut self, raw: &str) -> Result<()> {
        self.store_test();
        let input = self.get_input_file(None);
        println!("Raw test {}", input.display());
        std::fs::write(&input, raw)?;
        self.generate_answer(&input, &self.get_output_file(None))?;
        self.increase_test();
        Ok(())
    }

    pub fn copy_raw_test(&mut self, path: &Path) -> Result<()> {
        self.store_test();
        let input = self.get_input_file(None);
        std::fs::write(&input, std::fs::read(path)?)?;
        self.generate_answer(&input, &self.get_output_file(None))?;
        self.increase_test();
        Ok(())
    }

    pub fn generate_point_file(&self, point_file_path: &Path) -> Result<()> {
        let mut lines: Vec<(i32, i32, i32, String)> = Vec::new();
        let mut group_count = -1i32;
        for gr in &self.group_list {
            group_count += 1;
            if let Some(last) = lines.last_mut() {
                if last.2 == gr.0 && last.3 == gr.1 {
                    last.1 = group_count;
                } else {
                    lines.push((group_count, group_count, gr.0, gr.1.clone()));
                }
            } else {
                lines.push((group_count, group_count, gr.0, gr.1.clone()));
            }
        }
        let mut f = std::fs::File::create(point_file_path)?;
        for l in lines {
            writeln!(
                f,
                "{}-{} {}{}",
                l.0,
                l.1,
                l.2,
                if l.3.is_empty() { "" } else { " " }
            )?;
        }
        Ok(())
    }

    pub fn generate_test_description(&self, output: &Path) -> Result<()> {
        let mut f = std::fs::File::create(output)?;
        writeln!(
            f,
            "{:8}\t{:<5} {:<5} {:<8}",
            "Nr", "Grupa", "Gr Nr", "GPunkti"
        )?;
        let mut cnt = 0i32;
        for test in &self.test_list {
            cnt += 1;
            let grp = &self.group_list[test.0 as usize];
            writeln!(
                f,
                "{:8}\t{:5} {:5} {:8}\t{}",
                cnt, test.0, test.1, grp.0, grp.1
            )?;
        }
        Ok(())
    }

    pub fn generate_test_zip(&self, output: &Path, include_output: bool) -> Result<()> {
        let file = std::fs::File::create(output)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for test in &self.test_list {
            let input_file = self.get_input_file(Some(*test));
            let mut f = std::fs::File::open(&input_file)?;
            zip.start_file(input_file.file_name().unwrap().to_string_lossy(), options)?;
            std::io::copy(&mut f, &mut zip)?;
            if include_output {
                let output_file = self.get_output_file(Some(*test));
                let mut fo = std::fs::File::open(&output_file)?;
                zip.start_file(output_file.file_name().unwrap().to_string_lossy(), options)?;
                std::io::copy(&mut fo, &mut zip)?;
            }
        }
        zip.finish()?;
        println!(
            "Zipfile {} generated{}.",
            output.display(),
            if !include_output {
                " without output files"
            } else {
                ""
            }
        );
        Ok(())
    }
}
