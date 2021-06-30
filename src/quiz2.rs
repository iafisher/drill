use unicode_normalization::UnicodeNormalization;

use super::common::{QuizError, Result, TakeOptions};
use super::repetition2;
use super::ui2::CmdUI;

#[derive(Debug)]
pub struct Quiz2 {
    pub instructions: Option<String>,
    pub questions: Vec<Question2>,
    pub version: String,
}

#[derive(Debug)]
pub struct Question2 {
    pub id: i64,
    pub text: String,
    pub question_type: QuestionType,
    pub answers: Vec<Answer2>,
}

#[derive(Debug)]
pub enum QuestionType {
    ShortAnswer,
    Ordered,
    Unordered,
    MultipleChoice,
    Flashcard,
}

#[derive(Debug)]
pub struct Answer2 {
    pub variants: Vec<String>,
    pub correct: bool,
    pub no_credit: bool,
}

/// Represents the result of answering a question on a particular occasion.
#[derive(Debug, Clone)]
pub struct QuestionResult2 {
    pub id: i64,
    /// The text of the question exactly as it was asked. Optional for backwards
    /// compatibility.
    pub text: Option<String>,
    pub time_asked: chrono::DateTime<chrono::Utc>,
    /// If the question asked was a short answer question, then the user's response goes
    /// in this field.
    pub response: Option<String>,
    /// If the question asked was a list question, then the user's responses go in this
    /// field.
    pub response_list: Option<Vec<String>>,
    /// Score out of 1,000 possible points.
    pub score: u64,
}

/// Represents the results of taking a quiz on a particular occasion.
#[derive(Debug)]
pub struct QuizResult2 {
    pub time_finished: chrono::DateTime<chrono::Utc>,
    pub total: usize,
    pub total_correct: usize,
    pub total_partially_correct: usize,
    pub total_incorrect: usize,
    /// Score out of 1,000 possible points.
    pub score: u64,
    pub per_question: Vec<QuestionResult2>,
}

impl Quiz2 {
    pub fn take(&mut self, ui: &mut CmdUI, options: &TakeOptions) -> Result<QuizResult2> {
        let questions = repetition2::choose_questions(&self.questions, &options);
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
        let ret = QuizResult2 {
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

impl Question2 {
    fn ask(&self, ui: &mut CmdUI) -> Result<QuestionResult2> {
        ui.text(&self.text)?;
        if let Some(guess) = ui.prompt()? {
            if check(&self.answers[0], &guess) {
                ui.correct()?;
                let score = 1000;
                Ok(mkresult(self.id, &self.text, Some(guess), score))
            } else {
                ui.incorrect(Some(&self.answers[0].variants[0]))?;
                Ok(mkresult(self.id, &self.text, Some(guess), 0))
            }
        } else {
            ui.incorrect(Some(&self.answers[0].variants[0]))?;
            Ok(mkresult(self.id, &self.text, None, 0))
        }
    }
}

/// Construct a `QuestionResult` object.
fn mkresult(id: i64, text: &str, response: Option<String>, score: u64) -> QuestionResult2 {
    QuestionResult2 {
        id: id,
        text: Some(String::from(text)),
        time_asked: chrono::Utc::now(),
        score,
        response,
        response_list: None,
    }
}

/// Construct a `QuestionResult` object with a list of responses.
fn mkresultlist(id: i64, text: &str, responses: Vec<String>, score: u64) -> QuestionResult2 {
    QuestionResult2 {
        id: id,
        text: Some(String::from(text)),
        time_asked: chrono::Utc::now(),
        score,
        response: None,
        response_list: Some(responses),
    }
}

/// Return the index of the first answer in `answer_list` that `guess` matches, or
/// `None` if `guess` satisfies none.
pub fn check_one(answer_list: &Vec<Answer2>, guess: &str) -> Option<usize> {
    for (i, answer) in answer_list.iter().enumerate() {
        if check(answer, guess) {
            return Some(i);
        }
    }
    None
}

/// Return `true` if the given string is equivalent to the Answer object.
pub fn check(ans: &Answer2, guess: &str) -> bool {
    for variant in ans.variants.iter() {
        if normalize(&variant) == normalize(&guess) {
            return true;
        }
    }
    false
}

fn normalize(guess: &str) -> String {
    String::from(guess.to_lowercase()).nfc().collect::<String>()
}
