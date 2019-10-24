/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::io::Write;
use std::time;

use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;

use super::common::{QuizError, TakeOptions};
use super::iohelper::{prettyprint, prettyprint_colored, prompt};
use super::repetition;
use super::quiz;
use super::quiz::{Question, QuestionKind, QuestionResult, Quiz, QuizResult};


/// Take the quiz and return pairs of questions and results.
pub fn take(quiz: &mut Quiz, options: &TakeOptions) -> Result<QuizResult, QuizError> {
    if options.flip {
        for q in quiz.questions.iter_mut() {
            q.flip();
        }
    }

    let mut results = Vec::new();
    let mut total_correct = 0;
    let mut total_partially_correct = 0;
    let mut total = 0;
    let mut aggregate_score = 0.0;

    let questions = repetition::choose_questions(&quiz.questions, &options);
    if questions.len() == 0 {
        return Err(QuizError::EmptyQuiz);
    }

    if let Some(instructions) = &quiz.instructions {
        my_print!("\n")?;
        prettyprint_colored(
            &instructions, Some("  "), Some(Color::BrightBlue), None
        )?;
        my_print!("\n\n")?;
    }

    if questions.iter().any(|q| q.timeout.is_some()) {
        prettyprint_colored(
            "\nWarning: This quiz contains timed questions!",
            Some("  "),
            Some(Color::Red),
            None,
        )?;
        my_print!("\n")?;
    }

    for (i, question) in questions.iter().enumerate() {
        my_print!("\n")?;
        let result = ask(question, i+1);
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


/// Ask the question, get an answer, and return a `QuestionResult` object. If Ctrl+C
/// is pressed, return an error.
///
/// The `num` argument is the question number in the quiz, which is printed before
/// the text of the question.
pub fn ask(q: &Question, num: usize) -> Result<QuestionResult, QuizError> {
    let mut rng = thread_rng();
    let text = q.text.choose(&mut rng).unwrap();
    let prefix = format!("  ({}) ", num);
    prettyprint_colored(&text, Some(&prefix), None, Some(Color::Cyan))?;
    my_print!("\n")?;

    let now = time::Instant::now();
    match q.kind {
        QuestionKind::ShortAnswer | QuestionKind::Flashcard => {
            ask_short_answer(q, now)
        },
        QuestionKind::ListAnswer => {
            ask_list_answer(q)
        },
        QuestionKind::OrderedListAnswer => {
            ask_ordered_list_answer(q)
        }
        QuestionKind::MultipleChoice => {
            ask_multiple_choice(q, now)
        },
    }
}


/// Implementation of `ask` assuming that `q.kind` is `ShortAnswer` or `Flashcard`.
fn ask_short_answer(q: &Question, started: time::Instant) -> Result<QuestionResult, QuizError> {
    let guess = prompt("> ")?;
    let result = guess.is_some() && quiz::check_any(&q.answer_list, guess.as_ref().unwrap());
    let elapsed = started.elapsed();

    if result {
        correct(q, Some(elapsed))?;
    } else {
        let guess_option = guess.as_ref().map(|s| s.as_str());
        incorrect(q, Some(&q.answer_list[0][0]), guess_option)?;
    }

    let score = if result { score_with_timeout(q, elapsed) } else { 0.0 };

    if let Some(guess) = guess {
        Ok(mkresult(q, Some(guess), score))
    } else {
        Ok(mkresult(q, None, score))
    }
}


/// Implementation of `ask` assuming that `q.kind` is `ListAnswer`.
fn ask_list_answer(q: &Question) -> Result<QuestionResult, QuizError> {
    let mut satisfied = Vec::<bool>::with_capacity(q.answer_list.len());
    for _ in 0..q.answer_list.len() {
        satisfied.push(false);
    }

    let mut count = 0;
    let mut responses = Vec::new();
    while count < q.answer_list.len() {
        if let Some(guess) = prompt("> ")? {
            responses.push(guess.clone());

            if let Some(index) = quiz::check_one(&q.answer_list, &guess) {
                if satisfied[index] {
                    my_println!("You already said that.")?;
                } else {
                    satisfied[index] = true;
                    correct(q, None)?;
                    count += 1;
                }
            } else {
                if quiz::check(&q.no_credit, &guess) {
                    my_println!("No credit.")?;
                } else {
                    incorrect(q, None, Some(&guess))?;
                    count += 1;
                }
            }
        } else {
            incorrect(q, None, None)?;
            break;
        }
    }

    let ncorrect = satisfied.iter().filter(|x| **x).count();
    let score = (ncorrect as f64) / (q.answer_list.len() as f64);
    if ncorrect < q.answer_list.len() {
        my_println!("\nYou missed:")?;
        for (i, correct) in satisfied.iter().enumerate() {
            if !correct {
                my_println!("  {}", q.answer_list[i][0])?;
            }
        }
        my_println!(
            "\nScore for this question: {}", format!("{:.1}%", score * 100.0).cyan()
        )?;
    }

    Ok(mkresultlist(q, responses, score))
}


/// Implementation of `ask` assuming that `q.kind` is `OrderedListAnswer`.
fn ask_ordered_list_answer(q: &Question) -> Result<QuestionResult, QuizError> {
    let mut ncorrect = 0;
    let mut responses = Vec::new();
    for answer in q.answer_list.iter() {
        if let Some(guess) = prompt("> ")? {
            responses.push(guess.clone());

            if quiz::check(answer, &guess) {
                correct(q, None)?;
                ncorrect += 1;
            } else {
                incorrect(q, Some(&answer[0]), Some(&guess))?;
            }
        } else {
            incorrect(q, Some(&answer[0]), None)?;
            break;
        }
    }
    let score = (ncorrect as f64) / (q.answer_list.len() as f64);
    if score < 1.0 {
        my_println!(
            "\nScore for this question: {}", format!("{:.1}%", score * 100.0).cyan()
        )?;
    }

    Ok(mkresultlist(q, responses, score))
}


/// Implementation of `ask` assuming that `q.kind` is `MultipleChoice`.
fn ask_multiple_choice(q: &Question, started: time::Instant) -> Result<QuestionResult, QuizError> {
    let mut candidates = q.candidates.clone();

    let mut rng = thread_rng();
    // Shuffle once so that we don't always pick the first three candidates listed.
    candidates.shuffle(&mut rng);
    candidates.truncate(3);

    let answer = q.answer_list[0].choose(&mut rng).unwrap();
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
                let elapsed = started.elapsed();
                if quiz::check_any(&q.answer_list, guess) {
                    correct(q, Some(elapsed))?;
                    return Ok(
                        mkresult(
                            q,
                            Some(answer.clone()),
                            score_with_timeout(q, elapsed)
                        )
                    );
                } else {
                    incorrect(q, Some(&answer), Some(guess))?;
                    return Ok(mkresult(q, Some(answer.clone()), 0.0));
                }
            } else {
                continue;
            }
        } else {
            incorrect(q, Some(&answer), None)?;
            return Ok(mkresult(q, Some(answer.clone()), 0.0));
        }
    }
}


/// Construct a `QuestionResult` object.
fn mkresult(q: &Question, response: Option<String>, score: f64) -> QuestionResult {
    QuestionResult {
        id: q.id.clone(),
        time_asked: chrono::Utc::now(),
        score,
        response,
        response_list: None,
    }
}


/// Construct a `QuestionResult` object with a list of responses.
fn mkresultlist(q: &Question, responses: Vec<String>, score: f64) -> QuestionResult {
    QuestionResult {
        id: q.id.clone(),
        time_asked: chrono::Utc::now(),
        score,
        response: None,
        response_list: Some(responses),
    }
}


/// Print a message for correct answers.
fn correct(q: &Question, elapsed: Option<time::Duration>) -> Result<(), QuizError> {
    if let Some(elapsed) = elapsed {
        let score = score_with_timeout(q, elapsed);
        if score < 1.0 {
            my_print!("{}, but you exceeded the time limit. ", "Correct".green())?;
            my_println!(
                "Your score for this question is {}",
                format!("{:.1}%", score * 100.0).cyan()
            )?;
            return Ok(());
         }
    }
    my_println!("{}", "Correct!".green())
}


fn score_with_timeout(q: &Question, elapsed: time::Duration) -> f64 {
    if let Some(timeout) = q.timeout {
        let e = elapsed.as_millis() as i128;
        let t = (timeout * 1000) as i128;
        if e <= t {
            1.0
        } else if e < 2*t {
            (-1.0 / (t as f64)) * (e - 2 * t) as f64
        } else {
            0.0
        }
    } else {
        1.0
    }
}


/// Print a message for an incorrect answer, indicating that `answer` was the correct
/// answer.
fn incorrect(q: &Question, answer: Option<&str>, guess: Option<&str>) -> Result<(), QuizError> {
    let explanation = if let Some(guess) = guess {
        if let Some(explanation) = q.get_explanation(&guess) {
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
