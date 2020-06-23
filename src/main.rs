use anyhow::Result;
use git2::Repository;
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use structopt::StructOpt;

use code_insights::{Annotation, AnnotationBuilder, Annotations, ReportBuilder, Severity};

#[derive(StructOpt, Debug)]
struct Options {
    /// The Bitbucket base URL
    #[structopt(short, long)]
    url: String,

    /// The Bitbucket project key
    #[structopt(short, long)]
    project: String,

    /// The Bitbucket repository slug
    #[structopt(short, long)]
    slug: String,
}

fn main() -> Result<()> {
    let options = Options::from_args();

    let cwd = env::current_dir()?;
    let repository = Repository::discover(&cwd)?;
    let head = repository.refname_to_id("HEAD")?;

    let url = format!(
        "{}/rest/insights/1.0/projects/{}/repos/{}/commits/{}/reports/art.iculate.clippy",
        options.url, options.project, options.slug, head
    );

    println!("url is {}", url);

    let report = ReportBuilder::new("Clippy")
        .logo_url("https://www.rust-lang.org/logos/rust-logo-blk.svg")
        .build()?;

    let client = reqwest::blocking::Client::new();
    let res = client
        .put(&url)
        .basic_auth("admin", Some("admin"))
        .json(&report)
        .send()?;

    println!("{:?}", res);

    let url = format!(
        "{}/rest/insights/1.0/projects/{}/repos/{}/commits/{}/reports/art.iculate.clippy/annotations",
        options.url, options.project, options.slug, head
    );

    let annotations: Vec<Annotation> = run_clippy(&cwd)?
        .lines()
        .map(serde_json::from_str)
        .filter_map(Result::ok)
        .filter_map(to_annotation)
        .collect();

    println!("{:?}", annotations);

    let annotations = Annotations::new(annotations);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&url)
        .basic_auth("admin", Some("admin"))
        .json(&annotations)
        .send()?;

    println!("{:?}", res);

    Ok(())
}

fn run_clippy(dir: &PathBuf) -> Result<String> {
    let output = Command::new("cargo")
        .current_dir(dir)
        .arg("clippy")
        .arg("--message-format")
        .arg("json")
        .stderr(Stdio::null())
        .output()
        .expect("failed to run 'cargo clippy'");
    Ok(String::from_utf8(output.stdout)?)
}

fn to_annotation(json: Value) -> Option<Annotation> {
    if json["reason"] == "compiler-message" {
        let message = json["message"]["message"].as_str().unwrap();
        let severity = level_to_severity(json["message"]["level"].as_str().unwrap());

        let mut annotation = AnnotationBuilder::new(message, severity);

        if has_spans(&json) {
            let path = json["message"]["spans"][0]["file_name"].as_str().unwrap();
            let line = json["message"]["spans"][0]["line_start"].as_u64().unwrap() as u32;

            annotation = annotation.path(path).line(line);
        }

        return Some(annotation.build().unwrap());
    }
    None
}

fn has_spans(json: &Value) -> bool {
    !json["message"]["spans"].as_array().unwrap().is_empty()
}

fn level_to_severity(level: &str) -> Severity {
    match level {
        "note" | "help" => Severity::Low,
        "warning" => Severity::Medium,
        "error" => Severity::High,
        _ => Severity::Medium,
    }
}
