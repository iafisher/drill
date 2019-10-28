/**
 * The command-line user interface for taking quizzes.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::io::Write;
use std::process::Command;
use std::time;

use colored::*;

use super::common::{Location, QuizError, Result};
use super::iohelper::{prettyprint, prettyprint_colored, prompt};
use super::quiz::QuizResult;


pub struct CmdUI {
    number: usize,
    time_started: time::Instant,
}


impl CmdUI {
    pub fn new() -> Self {
        Self {
            number: 0,
            time_started: time::Instant::now(),
        }
    }

    pub fn next(&mut self) {
        self.time_started = time::Instant::now();
        self.number += 1;
    }

    pub fn text(&mut self, text: &str) -> Result<()> {
        my_print!("\n")?;
        let prefix = format!("  ({}) ", self.number);
        prettyprint_colored(&text, &prefix, None, Some(Color::Cyan))?;
        my_print!("\n")
    }

    pub fn prompt(&mut self) -> Result<Option<String>> {
        let response = prompt("> ")?;
        if let Some(response) = response.as_ref() {
            if response == "!!" {
                return Err(QuizError::SignalMarkCorrect);
            } else if "!edit".starts_with(response) {
                return Err(QuizError::SignalEdit);
            }
        }
        Ok(response)
    }

    pub fn incorrect(&mut self, correction: Option<&str>) -> Result<()> {
        if let Some(correction) = correction {
            let message = format!(
                "{} The correct answer was {}.",
                "Incorrect.".red(), 
                correction.green(),
            );
            prettyprint(&message, "")
        } else {
            prettyprint(&"Incorrect.".red(), "")
        }
    }

    pub fn correct(&mut self) -> Result<()> {
        prettyprint(&format!("{}", "Correct!".green()), "")
    }

    pub fn status(&mut self, text: &str) -> Result<()> {
        my_println!("{}", text)
    }

    pub fn score(&mut self, score: u64, timed_out: bool) -> Result<()> {
        let scorestr = format!("{:.1}%", (score as f64) / 10.0).cyan();
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
            prettyprint(choice, &prefix)?;
        }
        my_print!("\n")
    }

    pub fn get_elapsed(&self) -> time::Duration {
        self.time_started.elapsed()
    }

    pub fn instructions(&mut self, text: &str) -> Result<()> {
        my_print!("\n")?;
        prettyprint_colored(&text, "  ", Some(Color::BrightBlue), None)?;
        my_print!("\n")
    }

    pub fn warning(&mut self, text: &str) -> Result<()> {
        my_print!("\n")?;
        prettyprint_colored(
            &format!("Warning: {}", text), "  ", Some(Color::Red), None)?;
        my_print!("\n")
    }

    pub fn results(&mut self, results: &QuizResult) -> Result<()> {
        if results.total > 0 {
            let score_as_str = format!("{:.1}%", (results.score as f64) / 10.0);

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
            if results.total_partially_correct > 0 {
                my_print!(
                    "  {}",
                    format!("{}", results.total_partially_correct).bright_green()
                )?;
                my_print!(" partially correct\n")?;
            }
            my_print!("  {}", format!("{}", results.total_incorrect).red())?;
            my_print!(" incorrect\n")?;
        }
        Ok(())
    }

    pub fn launch_editor(&mut self, location: &Location) -> Result<()> {
        let editor = ::std::env::var("EDITOR").unwrap_or(String::from("nano"));
        let mut cmd = Command::new(&editor);
        cmd.arg(&location.path);

        if editor == "vim" {
            cmd.arg(format!("+{}", location.line));
        }

        let mut child = cmd.spawn().or(Err(QuizError::CannotOpenEditor))?;
        child.wait().or(Err(QuizError::CannotOpenEditor))?;
        Ok(())
    }
}
