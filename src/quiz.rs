/**
 * Implementation of the core quiz data structures.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::mem;

use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Serialize, Deserialize};

use super::common::{Location};


/// Represents an entire quiz.
#[derive(Debug)]
pub struct Quiz {
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
    /// Answers which count as neither correct nor incorrect. This field is only used
    /// when `kind` is set to `ListAnswer`.
    pub no_credit: Vec<String>,
    /// Prior results of answering the question.
    pub prior_results: Vec<QuestionResult>,
    /// User-defined tags for the question.
    pub tags: Vec<String>,
    /// Incorrect answers may be given specific explanations for why they are wrong.
    pub explanations: Vec<(Vec<String>, String)>,
    /// If specified, the number of seconds the user has to answer the question for full
    /// credit. Once passed, the user can still get partial credit up if she answers
    /// within `2*timeout` seconds.
    pub timeout: Option<u64>,
    /// Context for flashcards.
    pub front_context: Option<String>,
    pub back_context: Option<String>,

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
    // of this struct, but Rust's lifetimes make it more difficult than it's worth.
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
            no_credit: Vec::new(),
            prior_results: Vec::new(),
            explanations: Vec::new(),
            location: None,
            timeout: None,
            front_context: None,
            back_context: None,
        }
    }

    pub fn get_explanation(&self, guess: &str) -> Option<String> {
        let guess = guess.to_lowercase();
        for (variants, explanation) in self.explanations.iter() {
            if variants.contains(&guess) {
                return Some(explanation.clone());
            }
        }
        None
    }

    /// Flip flashcards. Does nothing if `self.kind` is not `Flashcard`.
    pub fn flip(&mut self) {
        // TODO: Handle this in parser instead?
        if self.kind == QuestionKind::Flashcard {
            let mut rng = thread_rng();
            self.answer_list.shuffle(&mut rng);

            let side1 = self.text.remove(0);
            let side2 = self.answer_list.remove(0).remove(0);

            self.text = vec![side2];
            self.answer_list = vec![vec![side1]];
            mem::swap(&mut self.front_context, &mut self.back_context);
        }
    }
}


/// Return `true` if `guess` matches any of the answers in `answer_list`.
pub fn check_any(answer_list: &Vec<Answer>, guess: &str) -> bool {
    for answer in answer_list.iter() {
        if check(answer, guess) {
            return true;
        }
    }
    false
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
    String::from(guess.to_lowercase())
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
