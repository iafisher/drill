/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
#[macro_use]
mod iohelper;
mod parser;
mod quiz;

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use colored::*;
use structopt::StructOpt;

use quiz::{QuestionResult, Quiz, QuizError, QuizResult};


fn main() {
    let options = parse_options();

    if let Err(e) = quiz::require_app_dir_path() {
        eprintln!("{}: {}", "Error".red(), e);
        ::std::process::exit(2);
    }

    let result = match options {
        quiz::QuizOptions::Take(options) => {
            main_take(options)
        },
        quiz::QuizOptions::Count(options) => {
            main_count(options)
        },
        quiz::QuizOptions::Results(options) => {
            main_results(options)
        },
        quiz::QuizOptions::Edit(options) => {
            main_edit(options)
        },
        quiz::QuizOptions::Rm(options) => {
            main_rm(options)
        },
        quiz::QuizOptions::Mv(options) => {
            main_mv(options)
        },
        quiz::QuizOptions::Ls(options) => {
            main_ls(options)
        },
        quiz::QuizOptions::Path(options) => {
            main_path(options)
        },
        quiz::QuizOptions::Git { args } => {
            main_git(args)
        }
    };

    if let Err(e) = result {
        if !quiz::is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}


/// The main function for the `take` subcommand.
pub fn main_take(options: quiz::QuizTakeOptions) -> Result<(), QuizError> {
    if options.no_color {
        colored::control::set_override(false);
    }

    let mut quiz = quiz::load_quiz(&options.name)?;
    let results = quiz.take(&options)?;
    output_results(&results)?;

    if results.total > 0 && (options.save || confirm("\nSave results? ")) {
        save_results(&options.name, &results)?;
    }
    Ok(())
}


fn output_results(results: &QuizResult) -> Result<(), QuizError> {
    if results.total > 0 {
        let score_as_str = format!("{:.1}%", results.score);

        my_print!("\n\n")?;
        my_print!("{}", "Score: ".white())?;
        my_print!("{}", score_as_str.cyan())?;
        my_print!("{}", " out of ".white())?;
        my_print!("{}", format!("{}", results.total).cyan())?;
        if results.total == 1 {
            my_println!("{}", " question".white())?;
        } else {
            my_println!("{}", " questions".white())?;
        }
        my_print!("  {}", format!("{}", results.total_correct).bright_green())?;
        my_print!("{}\n", " correct".white())?;
        my_print!("  {}", format!("{}", results.total_partially_correct).green())?;
        my_print!("{}\n", " partially correct".white())?;
        my_print!("  {}", format!("{}", results.total_incorrect).red())?;
        my_print!("{}\n", " incorrect".white())?;
    }
    Ok(())
}


/// The main function for the `count` subcommand.
pub fn main_count(options: quiz::QuizCountOptions) -> Result<(), QuizError> {
    let quiz = quiz::load_quiz(&options.name)?;
    if options.list_tags {
        list_tags(&quiz)?;
    } else {
        let filtered = quiz.filter_questions(&options.filter_opts);
        my_println!("{}", filtered.len())?;
    }
    Ok(())
}


/// The main function for the `results` subcommand.
pub fn main_results(options: quiz::QuizResultsOptions) -> Result<(), QuizError> {
    let results = quiz::load_results(&options.name)?;

    if results.len() == 0 {
        my_println!("No results have been recorded for this quiz.")?;
        return Ok(());
    }

    let mut aggregated: Vec<(f64, usize, String)> = Vec::new();
    for (key, result) in results.iter() {
        // Only include questions that have scored results.
        if let Some(score) = quiz::aggregate_results(&result) {
            aggregated.push((score, result.len(), key.clone()));
        }
    }

    if options.sort == "best" {
        aggregated.sort_by(cmp_results_best);
    } else if options.sort == "worst" {
        aggregated.sort_by(cmp_results_worst);
    } else if options.sort == "most" {
        aggregated.sort_by(cmp_results_most);
    } else if options.sort == "least" {
        aggregated.sort_by(cmp_results_least);
    } else {
    }

    if let Some(n) = options.num_to_show {
        aggregated.truncate(n);
    }

    for (score, attempts, question) in aggregated.iter() {
        let first_prefix = format!("{:>5.1}%  of {:>2}   ", score, attempts);
        iohelper::prettyprint_colored(&question, Some(&first_prefix), None, Some(Color::Cyan))?;
    }

    Ok(())
}


pub fn main_edit(options: quiz::QuizEditOptions) -> Result<(), QuizError> {
    let path = if options.results {
        quiz::get_results_path(&options.name)
    } else {
        quiz::get_quiz_path(&options.name)
    };

    loop {
        launch_editor(&path, None)?;

        if !options.results && path.exists() {
            // Parse it again to make sure it's okay.
            if let Err(e) = parser::parse(&path) {
                eprintln!("{}: {}", "Error".red(), e);
                if !confirm("Do you want to save anyway? ") {
                    continue;
                }
            }
        }
        break;
    }

    if !options.results && path.exists() && is_git_repo() {
        git(&["add", &path.as_path().to_string_lossy()])?;
        git(&["commit", "-m", &format!("Edit '{}'", options.name)])?;
    }

    Ok(())
}


/// Spawn an editor in a child process.
pub fn launch_editor(path: &PathBuf, line: Option<usize>) -> Result<(), QuizError> {
    let editor = ::std::env::var("EDITOR").unwrap_or(String::from("nano"));
    let mut cmd = Command::new(&editor);
    cmd.arg(&path);

    if editor == "vim" {
        if let Some(line) = line {
            cmd.arg(format!("+{}", line));
        } else {
            cmd.arg("+");
        }
    }

    let mut child = cmd.spawn().or(Err(QuizError::CannotOpenEditor))?;
    child.wait().or(Err(QuizError::CannotOpenEditor))?;
    Ok(())
}


pub fn main_rm(options: quiz::QuizRmOptions) -> Result<(), QuizError> {
    let path = quiz::get_quiz_path(&options.name);
    if path.exists() {
        let ask_prompt = "Are you sure you want to delete the quiz? ";
        if options.force || confirm(ask_prompt) {
            fs::remove_file(&path).map_err(QuizError::Io)?;
        }

        if is_git_repo() {
            git(&["rm", &path.as_path().to_string_lossy()])?;
            git(&["commit", "-m", &format!("Remove '{}'", options.name)])?;
        }

        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.clone()))
    }
}


pub fn main_mv(options: quiz::QuizMvOptions) -> Result<(), QuizError> {
    let quiz_path = quiz::get_quiz_path(&options.old_name);
    let new_quiz_path = quiz::get_quiz_path(&options.new_name);
    fs::rename(&quiz_path, &new_quiz_path).map_err(QuizError::Io)?;

    let results_path = quiz::get_results_path(&options.old_name);
    let new_results_path = quiz::get_results_path(&options.new_name);
    if results_path.exists() {
        fs::rename(&results_path, &new_results_path).map_err(QuizError::Io)?;
    }

    if is_git_repo() {
        git(&["rm", &quiz_path.as_path().to_string_lossy()])?;
        git(&["add", &new_quiz_path.as_path().to_string_lossy()])?;
        git(
            &[
                "commit",
                "-m",
                &format!("Rename '{}' to '{}'", options.old_name, options.new_name)
            ]
        )?;
    }

    Ok(())
}


pub fn main_ls(options: quiz::QuizLsOptions) -> Result<(), QuizError> {
    let mut dirpath = quiz::get_app_dir_path();
    dirpath.push("quizzes");

    let mut quiz_names = Vec::new();
    if let Ok(iter) = dirpath.read_dir() {
        for entry in iter {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    // For example, a .git entry.
                    if file_type.is_dir() {
                        continue;
                    }
                }

                if let Some(name) = entry.path().file_name() {
                    if name == ".gitignore" {
                        continue;
                    }
                    quiz_names.push(String::from(name.to_string_lossy()));
                }
            }
        }
    }

    if quiz_names.len() > 0 {
        quiz_names.sort_by(cmp_string_ignore_dot);
        my_println!("Available quizzes:")?;
        for name in quiz_names.iter() {
            if !name.starts_with(".") || options.all {
                my_println!("  {}", name)?;
            }
        }
    } else {
        my_println!("No quizzes found.")?;
    }

    Ok(())
}


pub fn main_path(options: quiz::QuizPathOptions) -> Result<(), QuizError> {
    let path = if options.results {
        quiz::get_results_path(&options.name)
    } else {
        quiz::get_quiz_path(&options.name)
    };

    if path.exists() || options.force {
        my_println!("{}", path.as_path().to_string_lossy())?;
        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.to_string()))
    }
}


pub fn main_git(args: Vec<String>) -> Result<(), QuizError> {
    let mut args_as_str = Vec::new();
    for arg in args.iter() {
        args_as_str.push(arg.as_str());
    }
    git(&args_as_str[..])
}


/// Parse command-line arguments.
pub fn parse_options() -> quiz::QuizOptions {
    let options = quiz::QuizOptions::from_args();

    if let quiz::QuizOptions::Results(options) = &options {
        let s = &options.sort;
        if s != "most" && s != "least" && s != "best" && s != "worst" {
            eprintln!("{}: unknown value `{}` for --sort.", "Error".red(), s);
            ::std::process::exit(2);
        }
    }

    options
}


/// Print a list of tags.
fn list_tags(quiz: &Quiz) -> Result<(), QuizError> {
    // Count how many times each tag has been used.
    let mut tags = HashMap::<&str, u32>::new();
    for question in quiz.questions.iter() {
        for tag in question.tags.iter() {
            if let Some(n) = tags.get(tag.as_str()) {
                tags.insert(tag.as_str(), n+1);
            } else {
                tags.insert(tag.as_str(), 1);
            }
        }
    }

    if tags.len() == 0 {
        my_println!("No questions have been assigned tags.")?;
    } else {
        my_println!("Available tags:")?;

        let mut tags_in_order: Vec<(&str, u32)> = tags.into_iter().collect();
        tags_in_order.sort();
        for (tag, count) in tags_in_order.iter() {
            my_println!("  {} ({})", tag, count)?;
        }
    }
    Ok(())
}


/// Save `results` to a file in the popquiz application's data directory, appending the
/// results if previous results have been saved.
fn save_results(name: &str, results: &QuizResult) -> Result<(), QuizError> {
    // Load old data, if it exists.
    let path = quiz::get_results_path(name);
    let data = fs::read_to_string(&path);
    let mut hash: BTreeMap<String, Vec<QuestionResult>> = match data {
        Ok(ref data) => {
            serde_json::from_str(&data)
                .map_err(QuizError::Json)?
        },
        Err(_) => {
            BTreeMap::new()
        }
    };

    // Store the results as a map from the text of the questions to a list of individual
    // time-stamped results.
    for result in results.per_question.iter() {
        if !hash.contains_key(&result.id) {
            hash.insert(result.id.to_string(), Vec::new());
        }
        hash.get_mut(&result.id).unwrap().push(result.clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .map_err(QuizError::Json)?;
    fs::write(&path, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(path.clone())))?;
    Ok(())
}


/// Return `true` if the quiz directory is a git repository.
fn is_git_repo() -> bool {
    let mut dirpath = quiz::get_quiz_dir_path();
    dirpath.push(".git");
    dirpath.exists()
}


fn git(args: &[&str]) -> Result<(), QuizError> {
    let dir = quiz::get_quiz_dir_path();
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .spawn()
        .or(Err(QuizError::CannotRunGit))?;
    child.wait().map_err(QuizError::Io)?;
    Ok(())
}


/// Prompt the user with a yes-no question and return `true` if they enter yes.
pub fn confirm(message: &str) -> bool {
    match quiz::prompt(message) {
        Ok(Some(response)) => {
            response.trim_start().to_lowercase().starts_with("y")
        },
        _ => false
    }
}


fn cmp_string_ignore_dot(a: &String, b: &String) -> Ordering {
    fn cmp_helper(a: &str, b: &str) -> Ordering {
        if a.starts_with(".") {
            cmp_helper(&a[1..], b)
        } else if b.starts_with(".") {
            cmp_helper(a, &b[1..])
        } else {
            a.cmp(b)
        }
    }

    cmp_helper(a, b)
}


/// An alias for a commonly-used typed in comparison functions.
type CmpQuestionResult = (f64, usize, String);


/// Comparison function that sorts an array of question results such that the best
/// results come first.
fn cmp_results_best(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    if a.0 < b.0 {
        return Ordering::Greater;
    } else if a.0 > b.0 {
        return Ordering::Less;
    } else {
        return cmp_results_most(a, b);
    }
}


/// Comparison function that sorts an array of question results such that the worst
/// results come first.
fn cmp_results_worst(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    return cmp_results_best(a, b).reverse();
}


/// Comparison function that sorts an array of question results such that the results
/// with the most attempts come first.
fn cmp_results_most(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    if a.1 < b.1 {
        return Ordering::Greater;
    } else if a.1 > b.1 {
        return Ordering::Less;
    } else {
        return Ordering::Equal;
    }
}


/// Comparison function that sorts an array of question results such that the results
/// with the least attempts come first.
fn cmp_results_least(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    return cmp_results_most(a, b).reverse();
}
