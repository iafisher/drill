use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use super::common::{QuizError, Result};

#[derive(Debug)]
pub struct Quiz2 {
    instructions: Option<String>,
    questions: Vec<Question2>,
    version: String,
}

#[derive(Debug)]
pub struct Question2 {
    text: String,
    question_type: QuestionType,
    answers: Vec<Answer2>,
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
    variants: Vec<String>,
    correct: bool,
    no_credit: bool,
}

pub fn load_quiz(fullname: &Path) -> Result<Quiz2> {
    // let exists = fullname.exists();
    // let connection = Connection::open(&path);
    let exists = false;
    let connection = Connection::open_in_memory().map_err(QuizError::Sql)?;
    if !exists {
        connection
            .execute(
                "
            CREATE TABLE quizzes(
              id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
              name TEXT UNIQUE NOT NULL CHECK(name != ''),
              instructions TEXT NOT NULL,
              version TEXT NOT NULL CHECK(version != ''),
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            ",
                [],
            )
            .map_err(QuizError::Sql)?;
        connection
            .execute(
                "
            CREATE TABLE questions(
              id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
              quiz INTEGER NOT NULL REFERENCES quizzes,
              text TEXT NOT NULL CHECK (text != ''),
              type TEXT NOT NULL CHECK(
                type = 'short answer' OR
                type = 'ordered' OR
                type = 'unordered' OR
                type = 'multiple choice' OR
                type = 'flashcard'
              ),
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            ",
                [],
            )
            .map_err(QuizError::Sql)?;
        connection
            .execute(
                "
            CREATE TABLE answers(
              question INTEGER NOT NULL REFERENCES questions,
              text TEXT NOT NULL CHECK(text != ''),
              correct BOOLEAN NOT NULL DEFAULT 1,
              no_credit BOOLEAN NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            ",
                [],
            )
            .map_err(QuizError::Sql)?;
    }

    connection
        .execute("INSERT INTO quizzes(name) VALUES ('main')", [])
        .map_err(QuizError::Sql)?;

    connection
        .execute(
            "
        INSERT INTO
          questions(quiz, type, text)
        VALUES
          (1, 'short answer', 'What is the capital of Tanzania?')
        ",
            [],
        )
        .map_err(QuizError::Sql)?;

    connection
        .execute(
            "
        INSERT INTO
          answers(question, text)
        VALUES
          (1, 'Dodoma')
        ",
            [],
        )
        .map_err(QuizError::Sql)?;

    connection
        .execute(
            "
        INSERT INTO
          answers(question, text)
        VALUES
          (1, 'Dar es Salaam')
        ",
            [],
        )
        .map_err(QuizError::Sql)?;

    let sql = "
            SELECT
              questions.id,
              questions.text,
              questions.type,
              answers.text,
              answers.correct,
              answers.no_credit
            FROM
              questions
            LEFT JOIN
              answers
            ON
              answers.question = questions.id
        ";

    let mut stmt = connection.prepare(sql).map_err(QuizError::Sql)?;
    let mut rows = stmt.query([]).map_err(QuizError::Sql)?;

    let mut questions_map = HashMap::new();
    while let Some(row) = rows.next().map_err(QuizError::Sql)? {
        let id = row.get_unwrap::<usize, i64>(0);
        if (!questions_map.contains_key(&id)) {
            let question_text = row.get_unwrap::<usize, String>(1);
            let question_type_string = row.get_unwrap::<usize, String>(2);
            let question_type = if question_type_string == "multiple choice" {
                QuestionType::MultipleChoice
            } else {
                QuestionType::ShortAnswer
            };
            questions_map.insert(
                id,
                Question2 {
                    text: question_text,
                    question_type: question_type,
                    answers: Vec::new(),
                },
            );
        }

        let mut question = questions_map.get_mut(&id).unwrap();

        let answer_text = row.get_unwrap::<usize, String>(3);
        let answer_correct = row.get_unwrap::<usize, i64>(4);
        let answer_no_credit = row.get_unwrap::<usize, i64>(5);
        question.answers.push(Answer2 {
            variants: vec![answer_text],
            correct: answer_correct != 0,
            no_credit: answer_no_credit != 0,
        });
    }

    Ok(Quiz2 {
        instructions: None,
        version: String::from("1.0"),
        // Courtesy of https://stackoverflow.com/questions/56724014
        questions: questions_map.into_iter().map(|(_id, q)| q).collect(),
    })
}
