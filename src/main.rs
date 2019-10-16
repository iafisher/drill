/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
mod common;
#[macro_use]
mod iohelper;
mod parser;
mod persistence;
mod quiz;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use colored::*;
use structopt::StructOpt;

use common::QuizError;
use iohelper::{confirm, prettyprint_colored};
use quiz::{Quiz, QuizResult};


fn main() {
    require_app_dir_path();

    let result = match parse_options() {
        common::QuizOptions::Take(options) => {
            main_take(options)
        },
        common::QuizOptions::Count(options) => {
            main_count(options)
        },
        common::QuizOptions::Results(options) => {
            main_results(options)
        },
        common::QuizOptions::Edit(options) => {
            main_edit(options)
        },
        common::QuizOptions::Rm(options) => {
            main_rm(options)
        },
        common::QuizOptions::Mv(options) => {
            main_mv(options)
        },
        common::QuizOptions::Ls(options) => {
            main_ls(options)
        },
        common::QuizOptions::Path(options) => {
            main_path(options)
        },
        common::QuizOptions::Search(options) => {
            main_search(options)
        },
        common::QuizOptions::Git { args } => {
            main_git(args)
        },
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}


/// The main function for the `take` subcommand.
pub fn main_take(options: common::QuizTakeOptions) -> Result<(), QuizError> {
    if options.no_color {
        colored::control::set_override(false);
    }

    let mut quiz = persistence::load_quiz(&options.name)?;
    let results = quiz.take(&options)?;
    output_results(&results)?;

    if results.total > 0 && (options.save || confirm("\nSave results? ")) {
        persistence::save_results(&options.name, &results)?;
    }
    Ok(())
}


fn output_results(results: &QuizResult) -> Result<(), QuizError> {
    if results.total > 0 {
        let score_as_str = format!("{:.1}%", results.score);

        my_print!("\n\n")?;
        my_print!("Score: ")?;
        my_print!("{}", score_as_str.cyan())?;
        my_print!(" out of ")?;
        my_print!("{}", format!("{}", results.total).cyan())?;
        if results.total == 1 {
            my_println!(" question")?;
        } else {
            my_println!(" questions")?;
        }
        my_print!("  {}", format!("{}", results.total_correct).green())?;
        my_print!(" correct\n")?;
        my_print!("  {}", format!("{}", results.total_partially_correct).bright_green())?;
        my_print!(" partially correct\n")?;
        my_print!("  {}", format!("{}", results.total_incorrect).red())?;
        my_print!(" incorrect\n")?;
    }
    Ok(())
}


/// The main function for the `count` subcommand.
pub fn main_count(options: common::QuizCountOptions) -> Result<(), QuizError> {
    let quiz = persistence::load_quiz(&options.name)?;
    if options.list_tags {
        list_tags(&quiz)?;
    } else {
        let filtered = quiz.filter_questions(&options.filter_opts);
        my_println!("{}", filtered.len())?;
    }
    Ok(())
}


/// The main function for the `results` subcommand.
pub fn main_results(options: common::QuizResultsOptions) -> Result<(), QuizError> {
    let quiz = persistence::load_quiz(&options.name)?;
    let results = persistence::load_results(&options.name)?;

    if results.len() == 0 {
        my_println!("No results have been recorded for this quiz.")?;
        return Ok(());
    }

    let mut aggregated: Vec<(f64, usize, String, String)> = Vec::new();
    for (key, result) in results.iter() {
        // Only include questions that have scored results.
        if let Some(score) = quiz::aggregate_results(&result) {
            if let Some(pos) = quiz.questions.iter().position(|q| q.id == *key) {
                let text = &quiz.questions[pos].text[0];
                aggregated.push((score, result.len(), key.clone(), text.clone()));
            }
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

    for (score, attempts, id, text) in aggregated.iter() {
        let first_prefix = format!("{:>5.1}%  of {:>2}   ", score, attempts);
        prettyprint_colored(
            &format!("[{}] {}", id, text), Some(&first_prefix), None, Some(Color::Cyan)
        )?;
    }

    Ok(())
}


pub fn main_edit(options: common::QuizEditOptions) -> Result<(), QuizError> {
    let path = if options.results {
        persistence::get_results_path(&options.name)
    } else {
        persistence::get_quiz_path(&options.name)
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


pub fn main_rm(options: common::QuizRmOptions) -> Result<(), QuizError> {
    let path = persistence::get_quiz_path(&options.name);
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


pub fn main_mv(options: common::QuizMvOptions) -> Result<(), QuizError> {
    let quiz_path = persistence::get_quiz_path(&options.old_name);
    let new_quiz_path = persistence::get_quiz_path(&options.new_name);
    fs::rename(&quiz_path, &new_quiz_path).map_err(QuizError::Io)?;

    let results_path = persistence::get_results_path(&options.old_name);
    let new_results_path = persistence::get_results_path(&options.new_name);
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


pub fn main_ls(options: common::QuizLsOptions) -> Result<(), QuizError> {
    let mut dirpath = persistence::get_app_dir_path();
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


pub fn main_path(options: common::QuizPathOptions) -> Result<(), QuizError> {
    let path = if options.results {
        persistence::get_results_path(&options.name)
    } else {
        persistence::get_quiz_path(&options.name)
    };

    if path.exists() || options.force {
        my_println!("{}", path.as_path().to_string_lossy())?;
        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.to_string()))
    }
}


pub fn main_search(options: common::QuizSearchOptions) -> Result<(), QuizError> {
    let quiz = persistence::load_quiz(&options.name)?;

    for question in quiz.questions.iter() {
        for text in question.text.iter() {
            if text.contains(&options.term) {
                prettyprint_colored(
                    &text, Some(&format!("[{}] ", question.id)), None, Some(Color::Cyan)
                )?;
                my_println!("")?;
                break;
            }
        }
    }

    Ok(())
}


pub fn main_git(args: Vec<String>) -> Result<(), QuizError> {
    let mut args_as_str = Vec::new();
    for arg in args.iter() {
        args_as_str.push(arg.as_str());
    }
    git(&args_as_str[..])
}


/// Parse command-line arguments.
pub fn parse_options() -> common::QuizOptions {
    let options = common::QuizOptions::from_args();

    if let common::QuizOptions::Results(options) = &options {
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


/// Return `true` if the quiz directory is a git repository.
fn is_git_repo() -> bool {
    let mut dirpath = persistence::get_quiz_dir_path();
    dirpath.push(".git");
    dirpath.exists()
}


fn git(args: &[&str]) -> Result<(), QuizError> {
    let dir = persistence::get_quiz_dir_path();
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .spawn()
        .or(Err(QuizError::CannotRunGit))?;
    child.wait().map_err(QuizError::Io)?;
    Ok(())
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
type CmpQuestionResult = (f64, usize, String, String);


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


/// Create the application directory if it doesn't already exist, or exit with an error
/// message if it does not exist and cannot be created.
pub fn require_app_dir_path() {
    if let Some(mut dirpath) = dirs::data_dir() {
        dirpath.push("iafisher_popquiz");
        if let Err(_) = make_directory(&dirpath) {
            cannot_make_app_dir();
        }

        dirpath.push("results");
        if let Err(_) = make_directory(&dirpath) {
            cannot_make_app_dir();
        }

        dirpath.pop();
        dirpath.push("quizzes");
        if let Err(_) = make_directory(&dirpath) {
            cannot_make_app_dir();
        }
    } else {
        cannot_make_app_dir();
    }
}


fn cannot_make_app_dir() {
    let path = String::from(persistence::get_app_dir_path().to_string_lossy());
    eprintln!("{}: unable to create application directory at {}", "Error".red(), path);
    ::std::process::exit(2);
}


fn make_directory(path: &PathBuf) -> Result<(), std::io::Error> {
    if !path.as_path().exists() {
        fs::create_dir(path)?;
    }
    Ok(())
}


fn is_broken_pipe(e: &QuizError) -> bool {
    if let QuizError::Io(e) = e {
        if let io::ErrorKind::BrokenPipe = e.kind() {
            return true;
        }
    }
    false
}
