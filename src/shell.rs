/**
 * The command-line user interface for taking quizzes.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::io::Write;
use std::time;

use colored::*;

use super::common::{QuizError, Result};
use super::iohelper::{prettyprint, prettyprint_colored, prompt};
use super::quiz::QuizResult;


pub struct CmdUI {
    number: usize,
    time_started: time::Instant,
    /// Have we finished printing out the prologue?
    finished_prologue: bool,
}


impl CmdUI {
    pub fn new() -> Self {
        Self {
            number: 0,
            time_started: time::Instant::now(),
            finished_prologue: false,
        }
    }

    pub fn text(&mut self, text: &str) -> Result<()> {
        if !self.finished_prologue {
            my_print!("\n")?;
            self.finished_prologue = true;
        }
        self.time_started = time::Instant::now();
        self.number += 1;

        let prefix = format!("  ({}) ", self.number);
        prettyprint_colored(&text, Some(&prefix), None, Some(Color::Cyan))?;
        my_print!("\n")
    }

    pub fn prompt(&mut self) -> Result<Option<String>> {
        prompt("> ")
    }

    pub fn incorrect(&mut self, correction: Option<&str>) -> Result<()> {
        if let Some(correction) = correction {
            let message = format!(
                "{} The correct answer was {}.",
                "Incorrect.".red(), 
                correction.green(),
            );
            prettyprint(&message, None)
        } else {
            prettyprint(&"Incorrect.".red(), None)
        }
    }

    pub fn correct(&mut self) -> Result<()> {
        prettyprint(&"Correct!".green(), None)
    }

    pub fn repeat(&mut self) -> Result<()> {
        my_println!("You already said that.")
    }

    pub fn no_credit(&mut self) -> Result<()> {
        my_println!("No credit.")
    }

    pub fn score(&mut self, score: f64, timed_out: bool) -> Result<()> {
        let scorestr = format!("{:.1}%", score * 100.0).cyan();
        if timed_out {
            my_println!("Score for this question: {} (exceeded time limit)", scorestr)
        } else {
            my_println!("Score for this question: {}", scorestr)
        }
    }

    pub fn missed(&mut self, missed: &Vec<&str>) -> Result<()> {
        my_println!("\nYou missed:")?;
        for m in missed.iter() {
            my_println!("  {}", m)?;
        }
        my_print!("\n")
    }

    pub fn choices(&mut self, choices: &Vec<&str>) -> Result<()> {
        for (i, choice) in "abcd".chars().zip(choices.iter()) {
            let prefix = format!("     ({}) ", i);
            prettyprint(choice, Some(&prefix))?;
        }
        my_print!("\n")
    }

    pub fn get_elapsed(&self) -> time::Duration {
        self.time_started.elapsed()
    }

    pub fn instructions(&mut self, text: &str) -> Result<()> {
        my_print!("\n")?;
        prettyprint_colored(&text, Some("  "), Some(Color::BrightBlue), None)?;
        my_print!("\n")
    }

    pub fn warning(&mut self, text: &str) -> Result<()> {
        my_print!("\n")?;
        prettyprint_colored(
            &format!("Warning: {}", text), Some("  "), Some(Color::Red), None)?;
        my_print!("\n")
    }

    pub fn results(&mut self, results: &QuizResult) -> Result<()> {
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
            my_print!(
                "  {}", format!("{}", results.total_partially_correct).bright_green())?;
            my_print!(" partially correct\n")?;
            my_print!("  {}", format!("{}", results.total_incorrect).red())?;
            my_print!(" incorrect\n")?;
        }
        Ok(())
    }
}
