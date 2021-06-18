/**
 * Implementation of the core quiz data structures.
 *
 * Author:  Ian Fisher (iafisher@fastmail.com)
 * Version: October 2019
 */
use std::mem;

use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use super::common::{Location, QuizError, Result, TakeOptions};
use super::repetition;
use super::ui::CmdUI;

/// Represents an entire quiz.
#[derive(Debug)]
pub struct Quiz {
    pub instructions: Option<String>,
    pub questions: Vec<Box<dyn Question>>,
}

impl Quiz {
    /// Find a question given its id.
    pub fn find(&self, id: &str) -> Option<usize> {
        self.questions.iter().position(|q| q.get_common().id == id)
    }

    pub fn take(&mut self, ui: &mut CmdUI, options: &TakeOptions) -> Result<QuizResult> {
        if options.flip {
            for q in self.questions.iter_mut() {
                q.flip();
            }
        }

        let questions = repetition::choose_questions(&self.questions, &options);
        if questions.len() == 0 {
            return Err(QuizError::EmptyQuiz);
        }

        if let Some(instructions) = &self.instructions {
            ui.instructions(&instructions)?;
        }

        let mut results = Vec::new();
        let mut index = 0;
        ui.next();
        while index < questions.len() {
            let result = questions[index].ask(ui);
            match result {
                Ok(result) => {
                    results.push(result);
                }
                Err(QuizError::ReadlineInterrupted) => {
                    break;
                }
                Err(QuizError::SignalMarkCorrect) => {
                    if results.len() > 0 {
                        let last = results.len() - 1;
                        if results[last].score < 1000 {
                            results[last].score = 1000;
                            ui.status("Previous answer marked correct.")?;
                        } else {
                            ui.status("Previous answer was already correct.")?;
                        }
                    } else {
                        ui.status("No previous question to mark correct.")?;
                    }
                    // Continue asking the same question.
                    continue;
                }
                Err(QuizError::SignalEdit) => {
                    if index > 0 {
                        let prev = &questions[index - 1];
                        ui.launch_editor(&prev.get_common().location)?;
                        ui.status(
                            "Edited previous question. Enter !! to mark your answer correct.",
                        )?;
                    } else {
                        ui.status("No previous question to edit.")?;
                    }
                    // Continue asking the same question.
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            }
            index += 1;
            ui.next();
        }

        let total = results.len();
        let aggregate_score: u64 = results.iter().map(|r| r.score).sum();
        let total_correct = results.iter().filter(|r| r.score == 1000).count();
        let total_partially_correct = results
            .iter()
            .filter(|r| r.score < 1000 && r.score > 0)
            .count();
        let total_incorrect = total - total_correct - total_partially_correct;
        let score = if total > 0 {
            aggregate_score / total as u64
        } else {
            0
        };
        let ret = QuizResult {
            time_finished: chrono::Utc::now(),
            total,
            total_correct,
            total_partially_correct,
            total_incorrect,
            score,
            per_question: results,
        };
        ui.results(&ret)?;
        Ok(ret)
    }
}

pub trait Question: std::fmt::Debug {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult>;
    fn get_common(&self) -> &QuestionCommon;
    fn get_text(&self) -> String;
    fn flip(&mut self) {}
}

#[derive(Debug, Clone)]
pub struct QuestionCommon {
    pub id: String,
    pub prior_results: Vec<QuestionResult>,
    pub tags: Vec<String>,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct ShortAnswerQuestion {
    pub text: String,
    pub answer: Answer,
    pub common: QuestionCommon,
}

impl Question for ShortAnswerQuestion {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult> {
        ui.text(&self.text)?;
        if let Some(guess) = ui.prompt()? {
            if check(&self.answer, &guess) {
                ui.correct()?;
                let score = 1000;
                Ok(mkresult(
                    &self.get_common().id,
                    &self.text,
                    Some(guess),
                    score,
                ))
            } else {
                ui.incorrect(Some(&self.answer[0]))?;
                Ok(mkresult(&self.get_common().id, &self.text, Some(guess), 0))
            }
        } else {
            ui.incorrect(Some(&self.answer[0]))?;
            Ok(mkresult(&self.get_common().id, &self.text, None, 0))
        }
    }

    fn get_common(&self) -> &QuestionCommon {
        &self.common
    }
    fn get_text(&self) -> String {
        self.text.clone()
    }
}

#[derive(Debug, Clone)]
pub struct FlashcardQuestion {
    pub front: Answer,
    pub back: Answer,
    pub front_context: Option<String>,
    pub back_context: Option<String>,
    pub common: QuestionCommon,
}

impl Question for FlashcardQuestion {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult> {
        let text = if let Some(context) = &self.front_context {
            format!("{} [{}]", self.front[0], context)
        } else {
            self.front[0].clone()
        };
        ui.text(&text)?;

        if let Some(guess) = ui.prompt()? {
            if check(&self.back, &guess) {
                ui.correct()?;
                let score = 1000;
                Ok(mkresult(&self.get_common().id, &text, Some(guess), score))
            } else {
                ui.incorrect(Some(&self.back[0]))?;
                Ok(mkresult(&self.get_common().id, &text, Some(guess), 0))
            }
        } else {
            ui.incorrect(Some(&self.back[0]))?;
            Ok(mkresult(&self.get_common().id, &text, None, 0))
        }
    }

    fn get_common(&self) -> &QuestionCommon {
        &self.common
    }
    fn get_text(&self) -> String {
        self.front[0].clone()
    }

    fn flip(&mut self) {
        mem::swap(&mut self.front, &mut self.back);
        mem::swap(&mut self.front_context, &mut self.back_context);
    }
}

#[derive(Debug, Clone)]
pub struct ListQuestion {
    pub text: String,
    pub answer_list: Vec<Answer>,
    pub no_credit: Vec<String>,
    pub common: QuestionCommon,
}

impl Question for ListQuestion {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult> {
        let n = self.answer_list.len();
        let mut satisfied = vec![false; n];

        ui.text(&self.text)?;
        let mut responses = Vec::new();
        while responses.len() < n {
            match ui.prompt() {
                Ok(Some(guess)) => {
                    if let Some(index) = check_one(&self.answer_list, &guess) {
                        if satisfied[index] {
                            ui.status("You already said that.")?;
                        } else {
                            satisfied[index] = true;
                            responses.push(guess.clone());
                            ui.correct()?;
                        }
                    } else {
                        if check(&self.no_credit, &guess) {
                            ui.status("No credit.")?;
                        } else {
                            responses.push(guess.clone());
                            ui.incorrect(None)?;
                        }
                    }
                }
                Ok(None) => {
                    ui.incorrect(None)?;
                    break;
                }
                Err(QuizError::SignalMarkCorrect) => {
                    if let Some(last) = responses.last().as_ref() {
                        if check_one(&self.answer_list, last).is_none() {
                            // We can't actually mark the answer correct without knowing
                            // which real answer it was meant to match, so instead we
                            // just undo the answer.
                            responses.pop();
                            ui.status("Previous answer undone.")?;
                        } else {
                            ui.status("Previous answer was already correct.")?;
                        }
                    } else {
                        // If there was no previous answer to this question, then we
                        // propagate the error upwards so that the previous question
                        // can be corrected.
                        return Err(QuizError::SignalMarkCorrect);
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        let mut missed = Vec::new();
        for (i, b) in satisfied.iter().enumerate() {
            if !b {
                missed.push(self.answer_list[i][0].as_str());
            }
        }

        if missed.len() > 0 {
            ui.missed(&missed)?;
        }
        let score = (n - missed.len()) as f64 / (n as f64);
        let score = (score * 1000.0) as u64;
        ui.score(score)?;

        Ok(mkresultlist(
            &self.get_common().id,
            &self.text,
            responses,
            score,
        ))
    }

    fn get_common(&self) -> &QuestionCommon {
        &self.common
    }
    fn get_text(&self) -> String {
        self.text.clone()
    }
}

#[derive(Debug, Clone)]
pub struct OrderedListQuestion {
    pub text: String,
    pub answer_list: Vec<Answer>,
    pub no_credit: Vec<String>,
    pub common: QuestionCommon,
}

impl Question for OrderedListQuestion {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult> {
        ui.text(&self.text)?;

        let mut index = 0;
        let mut ncorrect = 0;
        let mut responses = Vec::new();
        while index < self.answer_list.len() {
            let answer = &self.answer_list[index];
            match ui.prompt() {
                Ok(Some(guess)) => {
                    responses.push(guess.clone());

                    if check(answer, &guess) {
                        ui.correct()?;
                        ncorrect += 1;
                    } else {
                        ui.incorrect(Some(&answer[0]))?;
                    }
                    index += 1;
                }
                Ok(None) => {
                    ui.incorrect(Some(&answer[0]))?;
                    break;
                }
                Err(QuizError::SignalMarkCorrect) => {
                    if let Some(last) = responses.last().as_ref() {
                        if check_one(&self.answer_list, last).is_none() {
                            ncorrect += 1;
                            ui.status("Previous answer marked correct.")?;
                        } else {
                            ui.status("Previous answer was already correct.")?;
                        }
                    } else {
                        // If there was no previous answer to this question, then we
                        // propagate the error upwards so that the previous question
                        // can be corrected.
                        return Err(QuizError::SignalMarkCorrect);
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        let score = (score * 1000.0) as u64;
        ui.score(score)?;
        Ok(mkresultlist(
            &self.get_common().id,
            &self.text,
            responses,
            score,
        ))
    }

    fn get_common(&self) -> &QuestionCommon {
        &self.common
    }
    fn get_text(&self) -> String {
        self.text.clone()
    }
}

#[derive(Debug, Clone)]
pub struct MultipleChoiceQuestion {
    pub text: String,
    pub answer: Answer,
    pub choices: Vec<String>,
    pub common: QuestionCommon,
}

impl Question for MultipleChoiceQuestion {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult> {
        ui.text(&self.text)?;

        let mut choices: Vec<&str> = self.choices.iter().map(|s| s.as_str()).collect();
        let mut rng = thread_rng();
        // Shuffle once so that we don't always pick the first three candidates listed.
        choices.shuffle(&mut rng);
        choices.truncate(3);

        let answer = self.answer.choose(&mut rng).unwrap();
        choices.push(&answer);
        // Shuffle again so that the position of the correct answer is random.
        choices.shuffle(&mut rng);

        ui.choices(&choices)?;
        let mut response = None;
        let mut correct = false;
        loop {
            if let Some(guess) = ui.prompt()? {
                if guess.len() != 1 {
                    ui.status("Please enter a letter.")?;
                    continue;
                }

                let index = guess.to_ascii_lowercase().as_bytes()[0];
                if 97 <= index && index < 101 {
                    let guess = choices[(index - 97) as usize];
                    response.replace(String::from(guess));
                    if check(&self.answer, guess) {
                        ui.correct()?;
                        correct = true;
                    } else {
                        ui.incorrect(Some(&answer))?;
                    }
                    break;
                } else {
                    ui.status("Please enter a letter.")?;
                    continue;
                }
            } else {
                ui.incorrect(Some(&answer))?;
                break;
            }
        }
        let score = if correct { 1000 } else { 0 };
        Ok(mkresult(&self.get_common().id, &self.text, response, score))
    }

    fn get_common(&self) -> &QuestionCommon {
        &self.common
    }
    fn get_text(&self) -> String {
        self.text.clone()
    }
}

/// Each member of the vector should be an equivalent answer, e.g.
/// `vec!["Mount Everest", "Everest"]`, not different answers to the same question. The
/// first element of the vector is taken to be the canonical form of the answer for
/// display.
pub type Answer = Vec<String>;

/// Represents the result of answering a question on a particular occasion.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct QuestionResult {
    #[serde(skip)]
    pub id: String,
    /// The text of the question exactly as it was asked. Optional for backwards
    /// compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub time_asked: chrono::DateTime<chrono::Utc>,
    /// If the question asked was a short answer question, then the user's response goes
    /// in this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// If the question asked was a list question, then the user's responses go in this
    /// field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_list: Option<Vec<String>>,
    /// Score out of 1,000 possible points.
    pub score: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
}

/// Represents the results of taking a quiz on a particular occasion.
#[derive(Debug)]
pub struct QuizResult {
    pub time_finished: chrono::DateTime<chrono::Utc>,
    pub total: usize,
    pub total_correct: usize,
    pub total_partially_correct: usize,
    pub total_incorrect: usize,
    /// Score out of 1,000 possible points.
    pub score: u64,
    pub per_question: Vec<QuestionResult>,
}

/// Return the index of the first answer in `answer_list` that `guess` matches, or
/// `None` if `guess` satisfies none.
pub fn check_one(answer_list: &Vec<Answer>, guess: &str) -> Option<usize> {
    for (i, answer) in answer_list.iter().enumerate() {
        if check(answer, guess) {
            return Some(i);
        }
    }
    None
}

/// Return `true` if the given string is equivalent to the Answer object.
pub fn check(ans: &Answer, guess: &str) -> bool {
    for variant in ans.iter() {
        if normalize(&variant) == normalize(&guess) {
            return true;
        }
    }
    false
}

fn normalize(guess: &str) -> String {
    String::from(guess.to_lowercase()).nfc().collect::<String>()
}

/// Construct a `QuestionResult` object.
fn mkresult(id: &str, text: &str, response: Option<String>, score: u64) -> QuestionResult {
    QuestionResult {
        id: String::from(id),
        text: Some(String::from(text)),
        time_asked: chrono::Utc::now(),
        score,
        response,
        response_list: None,
        timed_out: None,
    }
}

/// Construct a `QuestionResult` object with a list of responses.
fn mkresultlist(id: &str, text: &str, responses: Vec<String>, score: u64) -> QuestionResult {
    QuestionResult {
        id: String::from(id),
        text: Some(String::from(text)),
        time_asked: chrono::Utc::now(),
        score,
        response: None,
        response_list: Some(responses),
        timed_out: None,
    }
}

const MAX_SCORE: u64 = 1000;
pub fn score_to_perc(score: u64) -> f64 {
    (score as f64) / (MAX_SCORE as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checking_answers_works() {
        let ans = vec![s("Barack Obama"), s("Obama")];

        assert!(check(&ans, "Barack Obama"));
        assert!(check(&ans, "barack obama"));
        assert!(check(&ans, "Obama"));
        assert!(check(&ans, "obama"));
        assert!(!check(&ans, "Mitt Romney"));
    }

    fn s(mystr: &str) -> String {
        String::from(mystr)
    }
}
