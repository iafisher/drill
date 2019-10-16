/**
 * Functions and data structures for reading and writing quiz and results files in the
 * filesystem.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use super::parser;
use super::quiz::{QuestionResult, Quiz, QuizError, QuizResult};

/// Load a `Quiz` object given its name.
pub fn load_quiz(name: &str) -> Result<Quiz, QuizError> {
    let path = get_quiz_path(name);
    let results_path = get_results_path(name);
    load_quiz_from_file(name, &path, &results_path)
}


/// Load a `Quiz` object from a file. The `name` argument is used only for nice error
/// messages.
fn load_quiz_from_file(
    name: &str, path: &PathBuf, results_path: &PathBuf
) -> Result<Quiz, QuizError> {
    let mut quiz = parser::parse(&path)?;

    // Attach previous results to the `Question` objects.
    let old_results = load_results_from_file(results_path)?;
    for question in quiz.questions.iter_mut() {
        if let Some(results) = old_results.get(&question.text[0]) {
            question.prior_results = results.clone();
        }
    }

    Ok(quiz)
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


pub fn load_results(name: &str) -> Result<StoredResults, QuizError> {
    let path = get_results_path(name);
    load_results_from_file(&path)
}


fn load_results_from_file(path: &PathBuf) -> Result<StoredResults, QuizError> {
    match fs::read_to_string(path) {
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
pub fn save_results(name: &str, results: &QuizResult) -> Result<(), QuizError> {
    // Load old data, if it exists.
    let path = get_results_path(name);
    let data = fs::read_to_string(&path);
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
    fs::write(&path, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(path.clone())))?;
    Ok(())
}


/// Return the path to the application directory, creating it and all necessary
/// subdirectories if they don't exist.
pub fn require_app_dir_path() -> Result<PathBuf, QuizError> {
    let mut dirpath = dirs::data_dir().unwrap();
    dirpath.push("iafisher_popquiz");
    make_directory(&dirpath).or(Err(QuizError::CannotMakeAppDir))?;

    dirpath.push("results");
    make_directory(&dirpath).or(Err(QuizError::CannotMakeAppDir))?;

    dirpath.pop();
    dirpath.push("quizzes");
    make_directory(&dirpath).or(Err(QuizError::CannotMakeAppDir))?;

    Ok(dirpath)
}


/// Return the path to the file where results are stored for the given quiz.
pub fn get_results_path(quiz_name: &str) -> PathBuf {
    let mut dirpath = get_app_dir_path();
    dirpath.push("results");
    dirpath.push(format!("{}_results.json", quiz_name));
    dirpath
}


/// Return the path to the file where the given quiz is stored.
pub fn get_quiz_path(quiz_name: &str) -> PathBuf {
    let mut dirpath = get_quiz_dir_path();
    dirpath.push(quiz_name);
    dirpath
}


/// Return the path to the application directory.
pub fn get_app_dir_path() -> PathBuf {
    let mut dirpath = dirs::data_dir().unwrap();
    dirpath.push("iafisher_popquiz");
    dirpath
}


/// Return the path to the quiz directory.
pub fn get_quiz_dir_path() -> PathBuf {
    let mut dirpath = get_app_dir_path();
    dirpath.push("quizzes");
    dirpath
}


fn make_directory(path: &PathBuf) -> Result<(), std::io::Error> {
    if !path.as_path().exists() {
        fs::create_dir(path)?;
    }
    Ok(())
}
