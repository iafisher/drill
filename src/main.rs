/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
mod common;
#[macro_use]
mod iohelper;
mod persistence;
mod quiz;
mod repetition;
mod ui;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::io::Write;

use colored::*;
use structopt::StructOpt;

use common::{Command, QuizError, Options, Result};
use iohelper::{confirm, prettyprint, prettyprint_colored};
use quiz::{QuestionResult, Quiz};
use ui::CmdUI;


fn main() {
    let options = parse_options();
    if options.no_color {
        colored::control::set_override(false);
    }

    let result = match options.cmd {
        Command::Count(options) => {
            main_count(&options)
        },
        Command::History(options) => {
            main_history(&options)
        },
        Command::Results(options) => {
            main_results(&options)
        },
        Command::Search(options) => {
            main_search(&options)
        },
        Command::Take(options) => {
            main_take(&options)
        },
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}


pub fn main_count(options: &common::CountOptions) -> Result<()> {
    let quiz = persistence::load_quiz(&options.name)?;
    if options.list_tags {
        list_tags(&quiz)?;
    } else {
        let mut count = 0;
        for question in quiz.questions.iter() {
            let tags = &question.get_common().tags;
            if repetition::filter_tags(tags, &options.filter_opts) {
                count += 1;
            }
        }
        my_println!("{}", count)?;
    }
    Ok(())
}



pub fn main_history(options: &common::HistoryOptions) -> Result<()> {
    let quiz = persistence::load_quiz(&options.name)?;
    if let Some(pos) = quiz.find(&options.id) {
        let q = &quiz.questions[pos];
        let prefix = format!("[{}] ", q.get_common().id);
        prettyprint_colored(&q.get_text(), &prefix, None, Some(Color::Cyan))?;
        my_print!("\n")?;

        let results = &q.get_common().prior_results;
        if results.len() > 0 {
            for result in results.iter() {
                let date = result.time_asked
                    .with_timezone(&chrono::Local)
                    .format("%F %l:%M %p");
                let score = colored_score(result.score);
                let prefix = if let Some(true) = result.timed_out {
                    format!("{}: {} (timeout) for ", date, score)
                } else {
                    format!("{}: {} for ", date, score)
                };
                prettyprint(&response(&result), &prefix)?;
            }

            my_print!("\n")?;
            print_stats(&results)?;
        } else {
            prettyprint("No results for this question.", "")?;
        }

    } else {
        prettyprint(&format!("No question with id '{}' found.", options.id), "")?;
    }
    Ok(())
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
        let score = quiz::score_to_perc(*score) * 100.0;
        let first_prefix = format!("{:>5.1}% of {:>2}   ", score, attempts);
        prettyprint_colored(
            &format!("[{}] {}", id, text), &first_prefix, None, Some(Color::Cyan)
        )?;
    }

    Ok(())
}


pub fn main_search(options: &common::SearchOptions) -> Result<()> {
    let quiz = persistence::load_quiz(&options.name)?;

    for question in quiz.questions.iter() {
        let text = question.get_text();
        if text.contains(&options.term) {
            prettyprint_colored(
                &text,
                &format!("[{}] ", question.get_common().id),
                None,
                Some(Color::Cyan),
            )?;
            my_println!("")?;
            break;
        }
    }

    Ok(())
}


/// The main function for the `take` subcommand.
pub fn main_take(options: &common::TakeOptions) -> Result<()> {
    let mut quiz = persistence::load_quiz(&options.name)?;
    let mut ui = CmdUI::new();
    let results = quiz.take(&mut ui, &options)?;

    if results.total > 0 && (options.save || confirm("\nSave results? ")) {
        persistence::save_results(&options.name, &results)?;
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
fn list_tags(quiz: &Quiz) -> Result<()> {
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
        let mut tags_in_order: Vec<(&str, u32)> = tags.into_iter().collect();
        tags_in_order.sort();
        for (tag, count) in tags_in_order.iter() {
            my_println!("{} ({})", tag, count)?;
        }
    }
    Ok(())
}


fn print_stats(results: &Vec<QuestionResult>) -> Result<()> {
    my_println!("Sample: {}", format!("{:>6}", results.len()).cyan())?;
    let mean = quiz::score_to_perc(results_mean(results).unwrap()) * 100.0;
    my_println!("Mean:   {}", format!("{:>5.1}%", mean).cyan())?;
    let median = quiz::score_to_perc(results_median(results)) * 100.0;
    my_println!("Median: {}", format!("{:>5.1}%", median).cyan())?;
    let max = quiz::score_to_perc(results_max(results)) * 100.0;
    my_println!("Max:    {}", format!("{:>5.1}%", max).cyan())?;
    let min = quiz::score_to_perc(results_min(results)) * 100.0;
    my_println!("Min:    {}", format!("{:>5.1}%", min).cyan())
}


/// An alias for a commonly-used typed in comparison functions.
type CmpQuestionResult = (u64, usize, String, String);


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


fn response(result: &QuestionResult) -> String {
    if let Some(response) = result.response.as_ref() {
        format!("'{}'", response)
    } else if let Some(response_list) = result.response_list.as_ref() {
        format!("'{}'", response_list.join(" / "))
    } else {
        String::from("<response not recorded>")
    }
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


fn results_median(results: &Vec<QuestionResult>) -> u64 {
    let mut results: Vec<u64> = results.iter().map(|r| r.score).collect();
    results.sort();
    if results.len() % 2 == 0 {
        (results[results.len() / 2] + results[(results.len() / 2) - 1]) / 2
    } else {
        results[results.len() / 2]
    }
}


fn results_max(results: &Vec<QuestionResult>) -> u64 {
    results.iter().map(|r| r.score).max().unwrap()
}


fn results_min(results: &Vec<QuestionResult>) -> u64 {
    results.iter().map(|r| r.score).min().unwrap()
}


fn colored_score(score: u64) -> ColoredString {
    let score = quiz::score_to_perc(score) * 100.0;
    if score >= 80.0 {
        format!("{:>5.1}%", score).green()
    } else if score <= 20.0 {
        format!("{:>5.1}%", score).red()
    } else {
        format!("{:>5.1}%", score).cyan()
    }
}
