/**
 * Functions and data structures for reading and writing quiz and results files in the
 * filesystem.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use super::common::QuizError;
use super::parser;
use super::quiz::{QuestionResult, Quiz, QuizResult};

/// Load a `Quiz` object given its name.
pub fn load_quiz(dir: &Path, name: &str) -> Result<Quiz, QuizError> {
    let mut dir_mutable = dir.to_path_buf();
    dir_mutable.push(name);
    let mut quiz = parser::parse(&dir_mutable)?;

    // Attach previous results to the `Question` objects.
    // TODO: Move this to parser.rs so Question can be immutable.
    let old_results = load_results(&dir, name)?;
    for question in quiz.questions.iter_mut() {
        if let Some(results) = old_results.get(&question.get_common().id) {
            question.get_common().prior_results = results.clone();
        }
    }

    Ok(quiz)
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


pub fn load_results(dir: &Path, name: &str) -> Result<StoredResults, QuizError> {
    let mut dir_mutable = dir.to_path_buf();
    dir_mutable.push("results");
    dir_mutable.push(format!("{}_results.json", name));
    match fs::read_to_string(dir_mutable) {
        Ok(data) => {
            serde_json::from_str(&data).map_err(QuizError::Json)
        },
        Err(_) => {
            Ok(HashMap::new())
        }
    }
}


/// Save `results` to a file in the popquiz application's data directory, appending the
/// results if previous results have been saved.
pub fn save_results(dir: &Path, name: &str, results: &QuizResult) -> Result<(), QuizError> {
    let mut dir_mutable = dir.to_path_buf();
    dir_mutable.push("results");
    if !dir_mutable.as_path().exists() {
        fs::create_dir(&dir_mutable).map_err(QuizError::Io)?;
    }

    // Load old data, if it exists.
    dir_mutable.push(format!("{}_results.json", name));
    let data = fs::read_to_string(&dir_mutable);
    let mut hash: BTreeMap<String, Vec<QuestionResult>> = match data {
        Ok(ref data) => {
            serde_json::from_str(&data)
                .map_err(QuizError::Json)?
        },
        Err(_) => {
            BTreeMap::new()
        }
    };

    // Store the results as a map from the text of the questions to a list of individual
    // time-stamped results.
    for result in results.per_question.iter() {
        if !hash.contains_key(&result.id) {
            hash.insert(result.id.to_string(), Vec::new());
        }
        hash.get_mut(&result.id).unwrap().push(result.clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .map_err(QuizError::Json)?;
    fs::write(&dir_mutable, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(dir_mutable.clone())))?;
    Ok(())
}
