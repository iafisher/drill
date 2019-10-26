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
mod repetition;
mod shell;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use colored::*;
use structopt::StructOpt;

use common::{Command, QuizError, Options};
use iohelper::{confirm, prettyprint_colored};
use quiz::Quiz;
use shell::CmdUI;


fn main() {
    let options = parse_options();
    if options.no_color {
        colored::control::set_override(false);
    }
    let directory = options.directory.unwrap_or(PathBuf::from("."));

    let result = match options.cmd {
        Command::Take(options) => {
            main_take(&directory.as_path(), options)
        },
        Command::Count(options) => {
            main_count(&directory.as_path(), options)
        },
        Command::Results(options) => {
            main_results(&directory.as_path(), options)
        },
        Command::Search(options) => {
            main_search(&directory.as_path(), options)
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
pub fn main_take(dir: &Path, options: common::TakeOptions) -> Result<(), QuizError> {
    let mut quiz = persistence::load_quiz(dir, &options.name)?;
    let mut ui = CmdUI::new();
    let results = quiz.take(&mut ui, &options)?;

    if results.total > 0 && (options.save || confirm("\nSave results? ")) {
        persistence::save_results(dir, &options.name, &results)?;
    }
    Ok(())
}


/// The main function for the `count` subcommand.
pub fn main_count(dir: &Path, options: common::CountOptions) -> Result<(), QuizError> {
    let quiz = persistence::load_quiz(dir, &options.name)?;
    if options.list_tags {
        list_tags(&quiz)?;
    } else {
        let mut count = 0;
        for question in quiz.questions.iter() {
            if !repetition::filter_tags(&question.get_common().tags, &options.filter_opts) {
                count += 1;
            }
        }
        my_println!("{}", count)?;
    }
    Ok(())
}


/// The main function for the `results` subcommand.
pub fn main_results(dir: &Path, options: common::ResultsOptions) -> Result<(), QuizError> {
    let quiz = persistence::load_quiz(dir, &options.name)?;
    let results = persistence::load_results(dir, &options.name)?;

    if results.len() == 0 {
        my_println!("No results have been recorded for this quiz.")?;
        return Ok(());
    }

    let mut aggregated: Vec<(f64, usize, String, String)> = Vec::new();
    for (key, result) in results.iter() {
        // Only include questions that have scored results.
        if let Some(score) = repetition::aggregate_results(&result) {
            if let Some(pos) = quiz.questions.iter().position(|q| q.get_common().id == *key) {
                let text = &quiz.questions[pos].get_text();
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


pub fn main_search(dir: &Path, options: common::SearchOptions) -> Result<(), QuizError> {
    let quiz = persistence::load_quiz(dir, &options.name)?;

    for question in quiz.questions.iter() {
        let text = question.get_text();
        if text.contains(&options.term) {
            prettyprint_colored(
                &text,
                Some(&format!("[{}] ", question.get_common().id)),
                None,
                Some(Color::Cyan),
            )?;
            my_println!("")?;
            break;
        }
    }

    Ok(())
}


/// Parse command-line arguments.
pub fn parse_options() -> common::Options {
    let options = Options::from_args();

    if let Command::Results(options) = &options.cmd {
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
        for tag in question.get_common().tags.iter() {
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


fn is_broken_pipe(e: &QuizError) -> bool {
    if let QuizError::Io(e) = e {
        if let io::ErrorKind::BrokenPipe = e.kind() {
            return true;
        }
    }
    false
}
