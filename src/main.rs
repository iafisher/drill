/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@fastmail.com)
 * Version: October 2019
 */
mod common;
#[macro_use]
mod iohelper;
mod persistence;
mod persistence2;
mod quiz;
mod quiz2;
mod repetition;
mod repetition2;
mod ui;
mod ui2;

use std::cmp::Ordering;
use std::env;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use colored::*;

use common::{Command, Options, QuizError, Result};
use iohelper::prettyprint_colored;
use quiz::QuestionResult;
use ui::CmdUI;

// fn main() {
fn main_v2() {
    let options = parse_options();
    if options.no_color {
        colored::control::set_override(false);
    }

    if let Ok(val) = env::var("DRILL_HOME") {
        if let Err(_) = env::set_current_dir(&val) {
            eprintln!("{}: could not cd to $DRILL_HOME ({})", "Error".red(), val);
            ::std::process::exit(2);
        }
    }

    let result = match options.cmd {
        Command::Results(options) => main_results_v2(&options),
        Command::Take(options) => main_take_v2(&options),
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}

fn main_results_v2(options: &common::ResultsOptions) -> Result<()> {
    Ok(())
}

fn main_take_v2(options: &common::TakeOptions) -> Result<()> {
    let mut quiz = persistence2::load_quiz(&options.name)?;
    let mut ui = ui2::CmdUI::new();
    let results = quiz.take(&mut ui, &options)?;

    // if results.total > 0 && !options.no_save {
    //     persistence::save_results(&options.name, &results)?;
    // }
    Ok(())
}

fn main() {
    let options = parse_options();
    if options.no_color {
        colored::control::set_override(false);
    }

    if let Ok(val) = env::var("DRILL_HOME") {
        if let Err(_) = env::set_current_dir(&val) {
            eprintln!("{}: could not cd to $DRILL_HOME ({})", "Error".red(), val);
            ::std::process::exit(2);
        }
    }

    let result = match options.cmd {
        Command::Results(options) => main_results(&options),
        Command::Take(options) => main_take(&options),
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}

/// The main function for the `results` subcommand.
pub fn main_results(options: &common::ResultsOptions) -> Result<()> {
    let quiz = persistence::load_quiz(&options.name)?;
    let results = persistence::load_results(&options.name)?;

    if results.len() == 0 {
        my_println!("No results have been recorded for this quiz.")?;
        return Ok(());
    }

    let mut aggregated: Vec<(u64, usize, String, String)> = Vec::new();
    for (key, result) in results.iter() {
        // Only include questions that have scored results.
        if let Some(score) = results_mean(&result) {
            if let Some(pos) = quiz.find(key) {
                let text = &quiz.questions[pos].get_text();
                aggregated.push((score, result.len(), key.clone(), text.clone()));
            }
        }
    }

    aggregated.sort_by(cmp_results_id);
    aggregated.sort_by(cmp_results_best);

    for (score, attempts, id, text) in aggregated.iter() {
        let score = quiz::score_to_perc(*score) * 100.0;
        let first_prefix = format!("{:>5.1}% of {:>2}   ", score, attempts);
        prettyprint_colored(
            &format!("[{}] {}", id, text),
            &first_prefix,
            None,
            Some(Color::Cyan),
        )?;
    }

    Ok(())
}

/// The main function for the `take` subcommand.
pub fn main_take(options: &common::TakeOptions) -> Result<()> {
    let mut quiz = persistence::load_quiz(&options.name)?;
    let mut ui = CmdUI::new();
    let results = quiz.take(&mut ui, &options)?;

    if results.total > 0 && !options.no_save {
        persistence::save_results(&options.name, &results)?;
    }
    Ok(())
}

/// Parse command-line arguments.
fn parse_options() -> common::Options {
    let mut args: Vec<String> = env::args().collect();
    args.remove(0);
    if args.len() == 0 {
        return Options {
            no_color: false,
            cmd: common::Command::Take(parse_take_options(&Vec::new())),
        };
    }

    let no_color = args[0] == "--no-color";
    if no_color {
        args.remove(0);
    }

    match args[0].as_str() {
        "--results" => {
            return Options {
                no_color,
                cmd: common::Command::Results(parse_results_options(&args)),
            };
        }
        "--take" => {
            return Options {
                no_color,
                cmd: common::Command::Take(parse_take_options(&args)),
            };
        }
        "-h" | "--help" => {
            println!("{}", HELP);
            ::std::process::exit(0);
        }
        _ => {
            return Options {
                no_color,
                cmd: common::Command::Take(parse_take_options(&args)),
            };
        }
    }
}

fn parse_results_options(args: &Vec<String>) -> common::ResultsOptions {
    let mut name = None;
    let mut i = 1;
    while i < args.len() {
        if args[i].starts_with("-") {
            cmd_error_unexpected_option(&args[i]);
        } else {
            if name.is_some() {
                cmd_error(&format!("Unexpected positional argument '{}'.", args[i]));
            } else {
                name.replace(PathBuf::from(&args[i]));
            }
            i += 1;
        }
    }

    common::ResultsOptions {
        name: name.unwrap_or(PathBuf::from("main")),
    }
}

fn parse_take_options(args: &Vec<String>) -> common::TakeOptions {
    let mut name = None;
    let mut flip = false;
    let mut in_order = false;
    let mut no_save = false;
    let mut num_to_ask = 20;
    let mut exclude = Vec::new();
    let mut tags = Vec::new();
    let mut i = if args.len() > 0 && args[0] == "--take" {
        1
    } else {
        0
    };
    while i < args.len() {
        if args[i] == "--flip" {
            flip = true;
            i += 1;
        } else if args[i] == "--in-order" {
            in_order = true;
            i += 1;
        } else if args[i] == "-n" {
            cmd_assert_next(args, i);
            if let Ok(n) = usize::from_str_radix(&args[i + 1], 10) {
                num_to_ask = n;
            } else {
                cmd_error("Expected integer argument to -n.");
            }
            i += 2;
        } else if args[i] == "--no-save" {
            no_save = true;
            i += 1;
        } else if args[i] == "--tag" {
            cmd_assert_next(args, i);
            tags.push(args[i + 1].clone());
            i += 2;
        } else if args[i] == "--exclude" {
            cmd_assert_next(args, i);
            exclude.push(args[i + 1].clone());
            i += 2;
        } else if args[i].starts_with("-") {
            cmd_error_unexpected_option(&args[i]);
        } else {
            if name.is_some() {
                cmd_error(&format!("Unexpected positional argument '{}'.", args[i]));
            } else {
                name.replace(PathBuf::from(&args[i]));
            }
            i += 1;
        }
    }

    common::TakeOptions {
        name: name.unwrap_or(PathBuf::from("main")),
        flip,
        in_order,
        no_save,
        num_to_ask,
        filter_opts: common::FilterOptions { exclude, tags },
    }
}

fn cmd_assert_next(args: &Vec<String>, i: usize) {
    if i == args.len() - 1 || args[i + 1].starts_with("-") {
        cmd_error(&format!("Option {} expected an argument.", args[i]));
    }
}

fn cmd_error_unexpected_option(option: &str) -> ! {
    cmd_error(&format!("Unexpected option {}.", option));
}

fn cmd_error(msg: &str) -> ! {
    eprintln!("{}", msg);
    eprintln!("\nRun drill --help for assistance.");
    ::std::process::exit(1);
}

/// An alias for a commonly-used typed in comparison functions.
/// (score, number of results, ID, question text)
type CmpQuestionResult = (u64, usize, String, String);

/// Comparison function that sorts an array of question results in alphabetical order
/// of ID.
fn cmp_results_id(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    if a.2 < b.2 {
        return Ordering::Less;
    } else if a.2 > b.2 {
        return Ordering::Greater;
    } else {
        return Ordering::Equal;
    }
}

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

fn is_broken_pipe(e: &QuizError) -> bool {
    if let QuizError::Io(e) = e {
        if let io::ErrorKind::BrokenPipe = e.kind() {
            return true;
        }
    }
    false
}

fn results_mean(results: &Vec<QuestionResult>) -> Option<u64> {
    if results.len() > 0 {
        // Tried to do this with iterators but Rust's type checker couldn't handle it.
        let mut sum = 0;
        for result in results.iter() {
            sum += result.score;
        }
        Some(sum / results.len() as u64)
    } else {
        None
    }
}

const HELP: &'static str = r"drill: quiz yourself from the command line.

Usage:
  drill <quiz>
  drill --results <quiz>
  drill --help

If <quiz> is not provided, it defaults to 'main' as long as the subcommand
requires no other positional argments.


take subcommand:
  --exclude <tag>    Exclude all questions with given tag.
  --flip             Flip all flashcards in the quiz.
  --in-order         Ask questions in the order they appear in the quiz file.
  -n <N>             Number of questions to ask. Defaults to 20.
  --no-save          Don't save results for this session.
  --tag <tag>        Include only questions with given tag.


results subcommand:
  <no special options>
";
