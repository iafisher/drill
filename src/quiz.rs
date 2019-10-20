/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::io::Write;

use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Serialize, Deserialize};

use super::common::{Location, QuizError, TakeOptions};
use super::iohelper::{prettyprint, prettyprint_colored, prompt};
use super::repetition;


/// Represents an entire quiz.
#[derive(Debug)]
pub struct Quiz {
    pub default_kind: Option<String>,
    pub instructions: Option<String>,
    pub questions: Vec<Question>,
}


/// Represents a question.
#[derive(Debug)]
pub struct Question {
    pub kind: QuestionKind,
    pub id: String,
    /// The text of the question. It is a vector instead of a string so that multiple
    /// variants of the same question can be stored.
    pub text: Vec<String>,
    /// Correct answers to the question. When `kind` is equal to `ShortAnswer` or
    /// `MultipleChoice`, this vector should have only one element.
    pub answer_list: Vec<Answer>,
    /// Candidate answers to the question. This field is only used when `kind` is set to
    /// `MultipleChoice`, in which case the candidates are incorrect answers to the
    /// question.
    pub candidates: Vec<String>,
    /// Prior results of answering the question.
    pub prior_results: Vec<QuestionResult>,
    /// User-defined tags for the question.
    pub tags: Vec<String>,
    /// Incorrect answers may be given specific explanations for why they are not
    /// right.
    pub explanations: Vec<(Vec<String>, String)>,

    /// The location where the question is defined.
    pub location: Option<Location>,
}



/// An enumeration for the `kind` field of `Question` objects.
#[derive(Debug, PartialEq, Eq)]
pub enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice, Flashcard,
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
    pub time_asked: chrono::DateTime<chrono::Utc>,
    /// If the question asked was a short answer question, then the user's response goes
    /// in this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// If the question asked was a list question, then the user's responses go in this
    /// field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_list: Option<Vec<String>>,
    pub score: f64,

    // It would be convenient to include a reference to the `Question` object as a field
    // of this struct, but Rust's lifetime makes it more difficult than it's worth.
}


/// Represents the results of taking a quiz on a particular occasion.
#[derive(Debug)]
pub struct QuizResult {
    pub time_finished: chrono::DateTime<chrono::Utc>,
    pub total: usize,
    pub total_correct: usize,
    pub total_partially_correct: usize,
    pub total_incorrect: usize,
    pub score: f64,
    pub per_question: Vec<QuestionResult>,
}


impl Quiz {
    /// Take the quiz and return pairs of questions and results.
    pub fn take(&mut self, options: &TakeOptions) -> Result<QuizResult, QuizError> {
        if options.flip {
            for q in self.questions.iter_mut() {
                q.flip();
            }
        }

        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total_partially_correct = 0;
        let mut total = 0;
        let mut aggregate_score = 0.0;

        let questions = repetition::choose_questions(&self.questions, &options);
        if questions.len() == 0 {
            return Err(QuizError::EmptyQuiz);
        }

        if let Some(instructions) = &self.instructions {
            my_print!("\n")?;
            prettyprint_colored(
                &instructions, Some("  "), Some(Color::BrightBlue), None
            )?;
            my_print!("\n\n")?;
        }

        for (i, question) in questions.iter().enumerate() {
            my_print!("\n")?;
            let result = question.ask(i+1);
            if let Ok(result) = result {
                let score = result.score;
                results.push(result);

                total += 1;
                aggregate_score += score;
                if score == 1.0 {
                    total_correct += 1;
                } else if score > 0.0 {
                    total_partially_correct += 1;
                }
            } else if let Err(QuizError::ReadlineInterrupted) = result {
                break;
            } else if let Err(e) = result {
                return Err(e);
            }
        }

        let total_incorrect = total - total_correct - total_partially_correct;
        let score = (aggregate_score / (total as f64)) * 100.0;
        Ok(QuizResult {
            time_finished: chrono::Utc::now(),
            total,
            total_correct,
            total_partially_correct,
            total_incorrect,
            score,
            per_question: results,
        })
    }
}


impl Question {
    /// Return a new short-answer question.
    #[allow(dead_code)]
    pub fn new(text: &str, answer: &str) -> Self {
        let answers = vec![vec![String::from(answer)]];
        Question {
            kind: QuestionKind::ShortAnswer,
            id: String::from("1"),
            text: vec![String::from(text)],
            tags: Vec::new(),
            answer_list: answers,
            candidates: Vec::new(),
            prior_results: Vec::new(),
            explanations: Vec::new(),
            location: None,
        }
    }

    /// Ask the question, get an answer, and return a `QuestionResult` object. If Ctrl+C
    /// is pressed, return an error.
    ///
    /// The `num` argument is the question number in the quiz, which is printed before
    /// the text of the question.
    pub fn ask(&self, num: usize) -> Result<QuestionResult, QuizError> {
        let mut rng = thread_rng();
        let text = self.text.choose(&mut rng).unwrap();
        let prefix = format!("  ({}) ", num);
        prettyprint_colored(&text, Some(&prefix), None, Some(Color::Cyan))?;
        my_print!("\n")?;

        match self.kind {
            QuestionKind::ShortAnswer | QuestionKind::Flashcard => {
                self.ask_short_answer()
            },
            QuestionKind::ListAnswer => {
                self.ask_list_answer()
            },
            QuestionKind::OrderedListAnswer => {
                self.ask_ordered_list_answer()
            }
            QuestionKind::MultipleChoice => {
                self.ask_multiple_choice()
            },
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ShortAnswer` or
    /// `Flashcard`.
    fn ask_short_answer(&self) -> Result<QuestionResult, QuizError> {
        let guess = prompt("> ")?;
        let result = guess.is_some() && self.check_any(guess.as_ref().unwrap());

        if result {
            self.correct()?;
        } else {
            let guess_option = guess.as_ref().map(|s| s.as_str());
            self.incorrect(Some(&self.answer_list[0][0]), guess_option)?;
        }

        let score = if result { 1.0 } else { 0.0 };

        if let Some(guess) = guess {
            Ok(self.result(Some(guess), score))
        } else {
            Ok(self.result(None, score))
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ListAnswer`.
    fn ask_list_answer(&self) -> Result<QuestionResult, QuizError> {
        let mut satisfied = Vec::<bool>::with_capacity(self.answer_list.len());
        for _ in 0..self.answer_list.len() {
            satisfied.push(false);
        }

        let mut count = 0;
        let mut responses = Vec::new();
        while count < self.answer_list.len() {
            if let Some(guess) = prompt("> ")? {
                responses.push(guess.clone());

                let index = self.check_one(&guess);
                if index == self.answer_list.len() {
                    self.incorrect(None, Some(&guess))?;
                    count += 1;
                } else if satisfied[index] {
                    my_println!("You already said that.")?;
                } else {
                    satisfied[index] = true;
                    self.correct()?;
                    count += 1;
                }
            } else {
                self.incorrect(None, None)?;
                break;
            }
        }

        let ncorrect = satisfied.iter().filter(|x| **x).count();
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if ncorrect < self.answer_list.len() {
            my_println!("\nYou missed:")?;
            for (i, correct) in satisfied.iter().enumerate() {
                if !correct {
                    my_println!("  {}", self.answer_list[i][0])?;
                }
            }
            my_println!(
                "\nScore for this question: {}", format!("{:.1}%", score * 100.0).cyan()
            )?;
        }

        Ok(self.result_with_list(responses, score))
    }

    /// Implementation of `ask` assuming that `self.kind` is `OrderedListAnswer`.
    fn ask_ordered_list_answer(&self) -> Result<QuestionResult, QuizError> {
        let mut ncorrect = 0;
        let mut responses = Vec::new();
        for answer in self.answer_list.iter() {
            if let Some(guess) = prompt("> ")? {
                responses.push(guess.clone());

                if check(answer, &guess) {
                    self.correct()?;
                    ncorrect += 1;
                } else {
                    self.incorrect(Some(&answer[0]), Some(&guess))?;
                }
            } else {
                self.incorrect(Some(&answer[0]), None)?;
                break;
            }
        }
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if score < 1.0 {
            my_println!(
                "\nScore for this question: {}", format!("{:.1}%", score * 100.0).cyan()
            )?;
        }

        Ok(self.result_with_list(responses, score))
    }

    /// Implementation of `ask` assuming that `self.kind` is `MultipleChoice`.
    fn ask_multiple_choice(&self) -> Result<QuestionResult, QuizError> {
        let mut candidates = self.candidates.clone();

        let mut rng = thread_rng();
        // Shuffle once so that we don't always pick the first three candidates listed.
        candidates.shuffle(&mut rng);
        candidates.truncate(3);

        let answer = self.answer_list[0].choose(&mut rng).unwrap();
        candidates.push(answer.clone());
        // Shuffle again so that the position of the correct answer is random.
        candidates.shuffle(&mut rng);

        for (i, candidate) in "abcd".chars().zip(candidates.iter()) {
            let prefix = format!("     ({}) ", i);
            prettyprint(candidate, Some(&prefix))?;
        }

        my_print!("\n")?;
        loop {
            if let Some(guess) = prompt("Enter a letter: ")? {
                if guess.len() != 1 {
                    continue;
                }

                let index = guess.to_ascii_lowercase().as_bytes()[0];
                if 97 <= index && index < 101 {
                    let guess = &candidates[(index - 97) as usize];
                    if self.check_any(guess) {
                        self.correct()?;
                        return Ok(self.result(Some(answer.clone()), 1.0));
                    } else {
                        self.incorrect(Some(&answer), Some(guess))?;
                        return Ok(self.result(Some(answer.clone()), 0.0));
                    }
                } else {
                    continue;
                }
            } else {
                self.incorrect(Some(&answer), None)?;
                return Ok(self.result(Some(answer.clone()), 0.0));
            }
        }
    }

    /// Construct a `QuestionResult` object.
    fn result(&self, response: Option<String>, score: f64) -> QuestionResult {
        QuestionResult {
            id: self.id.clone(),
            time_asked: chrono::Utc::now(),
            score,
            response,
            response_list: None,
        }
    }

    /// Construct a `QuestionResult` object with a list of responses.
    fn result_with_list(&self, responses: Vec<String>, score: f64) -> QuestionResult {
        QuestionResult {
            id: self.id.clone(),
            time_asked: chrono::Utc::now(),
            score,
            response: None,
            response_list: Some(responses),
        }
    }

    /// Print a message for correct answers.
    fn correct(&self) -> Result<(), QuizError> {
        my_println!("{}", "Correct!".green())
    }

    /// Print a message for an incorrect answer, indicating that `answer` was the
    /// correct answer.
    fn incorrect(
        &self, answer: Option<&str>, guess: Option<&str>
    ) -> Result<(), QuizError> {
        let explanation = if let Some(guess) = guess {
            if let Some(explanation) = self.get_explanation(&guess) {
                format!(" {}", explanation)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if let Some(answer) = answer {
            let message = format!(
                "{} The correct answer was {}.{}",
                "Incorrect.".red(),
                answer.green(),
                explanation
            );
            prettyprint(&message, None)?;
        } else {
            prettyprint(&format!("{}{}", "Incorrect.".red(), &explanation), None)?;
        }
        Ok(())
    }

    fn get_explanation(&self, guess: &str) -> Option<String> {
        let guess = guess.to_lowercase();
        for (variants, explanation) in self.explanations.iter() {
            if variants.contains(&guess) {
                return Some(explanation.clone());
            }
        }
        None
    }

    /// Return `true` if `guess` matches any of the answers in `self.answer_list`.
    fn check_any(&self, guess: &str) -> bool {
        for answer in self.answer_list.iter() {
            if check(answer, guess) {
                return true;
            }
        }
        false
    }

    /// Return the index of the first answer in `self.answer_list` that `guess`
    /// matches, or `self.answer_list.len()` if `guess` satisfies none.
    fn check_one(&self, guess: &str) -> usize {
        for (i, answer) in self.answer_list.iter().enumerate() {
            if check(answer, guess) {
                return i;
            }
        }
        self.answer_list.len()
    }

    /// Flip flashcards. Does nothing if `self.kind` is not `Flashcard`.
    fn flip(&mut self) {
        if self.kind == QuestionKind::Flashcard {
            let mut rng = thread_rng();
            self.answer_list.shuffle(&mut rng);

            let side1 = self.text.remove(0);
            let side2 = self.answer_list.remove(0).remove(0);

            self.text = vec![side2];
            self.answer_list = vec![vec![side1]];
        }
    }
}


/// Return `true` if the given string is equivalent to the Answer object.
fn check(ans: &Answer, guess: &str) -> bool {
    for variant in ans.iter() {
        if variant.to_lowercase() == guess.to_lowercase() {
            return true;
        }
    }
    false
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
