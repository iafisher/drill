use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time;


#[test]
fn can_take_simple_quiz1() {
    let (stdout, _) = spawn_and_mock("test1", &["Ulan Bator", "no"], &[]);
    assert_in_order(
        &stdout,
        &[
            "What is the capital of Mongolia?",
            "100.0%",
            "1 correct",
            "0 partially correct",
            "0 incorrect",
        ]
    );
}


#[test]
fn can_take_simple_quiz2() {
    let (stdout, _) = spawn_and_mock(
        "test2", &["a", "Wilhelm I", "Wilhelm II", "Wilhelm II"], &["--in-order"],
    );

    assert_in_order(
        &stdout,
        &[
            "Who was President of the United States during the Korean War?",
            "List the modern Emperors of Germany in chronological order.",
            "Incorrect. The correct answer was Frederick III.",
            "Score for this question: 66.7%",
            "1 partially correct",
        ]
    );

    // Since the order of multiple-choice answers is random, we don't know whether
    // guessing 'a' was right or not.
    assert!(stdout.contains("1 incorrect") || stdout.contains("1 correct"));
}

#[test]
fn can_take_quiz_with_list_question() {
    let (stdout, _) = spawn_and_mock(
        "test_list",
        &["China", "PR China", "France", "Germany", "US", "United Kingdom", "no"],
        &["--in-order"],
    );

    assert_in_order(
        &stdout,
        &[
            "Name the five members of the UN Security Council.",
            "Correct!\n",
            "You already said that.\n",
            "Correct!\n",
            "Incorrect.\n",
            "Correct!\n",
            "You missed:\n  Russia\n\n",
            "Score for this question: 80.0%",
        ],
    );
}

#[test]
fn can_take_flashcard_quiz() {
    let (stdout, _) = spawn_and_mock(
        "test_flashcard", &["bread", "wine", "butter", "no"], &["--in-order"],
    );

    assert_in_order(
        &stdout,
        &[
            "el pan",
            "el vino",
            "la mantequilla",
            "100.0%",
        ]
    );
}

#[test]
fn can_take_flipped_flashcard_quiz() {
    let (stdout, _) = spawn_and_mock(
        "test_flashcard",
        &["el pan", "el vino", "la mantequilla", "no"],
        &["--in-order", "--flip"],
    );

    assert_in_order(
        &stdout,
        &[
            "bread",
            "wine",
            "butter",
            "100.0%",
        ]
    );
}

#[test]
fn no_credit_answers_work() {
    let (stdout, _) = spawn_and_mock(
        "test_no_credit",
        &["Riverside", "Ontario", "San Bernardino", "Corona", "Fontana", "no"],
        &[],
    );

    assert_in_order(
        &stdout,
        &[
            "Name the three largest cities of the Inland Empire.",
            "Correct",
            "No credit",
            "Correct",
            "No credit",
            "Correct",
            "100.0%",
        ]
    );
}

#[test]
fn quiz_instructions_are_displayed() {
    let (stdout, _) = spawn_and_mock("test_instructions", &["Lansing, MI", "no"], &[]);

    assert_in_order(
        &stdout,
        &[
            "Include the state's postal code.",
            "Correct",
            "100.0%",
        ]
    );
}

#[test]
fn flashcards_context() {
    let (stdout, _) = spawn_and_mock("test_flashcard_context", &["прочитать", "no"], &[]);

    assert_in_order(
        &stdout,
        &[
            "to read [perf]",
            "Correct",
            "100.0%",
        ]
    );

    let (stdout, _) = spawn_and_mock(
        "test_flashcard_context", &["to read", "no"], &["--flip"]
    );

    assert_in_order(
        &stdout,
        &[
            "прочитать [bleh]",
            "Correct",
            "100.0%",
        ]
    );
}

#[test]
fn timeouts_work() {
    // This test can't use `spawn_and_mock` because it needs to control how long the
    // thread sleeps between answering questions.
    let mut process = spawn("test_timeouts", &["--in-order"]);
    let stdin = process.stdin.as_mut().expect("Failed to open stdin");
    stdin_write(stdin, "Chisinau");
    sleep(1200);
    stdin_write(stdin, "Kiev");
    sleep(1200);
    stdin_write(stdin, "Sardinia");
    stdin_write(stdin, "Sicily");

    let result = process.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();

    assert_in_order(
        &stdout,
        &[
            "Warning: This quiz contains timed questions!",
            "Correct!\n",
            "Correct, but you exceeded the time limit",
            "Correct!\n",
            "Correct!\n",
            // Make sure we got full credit for the list question.
            "2 correct",
            "1 partially correct",
        ],
    );
}

#[test]
fn parse_error_no_blank_line_between_questions() {
    assert_parse_error("test_no_blank_line", "no blank line between questions", 2, false);
}

#[test]
fn parse_error_no_blank_line_after_settings() {
    assert_parse_error(
        "test_no_blank_line_after_settings", "no blank line after quiz settings", 2, false,
    );
}

#[test]
fn parse_error_wrong_ordered_value() {
    assert_parse_error(
        "test_wrong_ordered_value", "ordered field must be either 'true' or 'false'", 1, true,
    );
}

#[test]
fn parse_error_no_first_line() {
    assert_parse_error("test_no_first_line", "expected first line of question", 1, false);
}

#[test]
fn parse_error_bad_attribute() {
    assert_parse_error("test_bad_attribute", "expected colon", 3, false);
}

#[test]
fn parse_error_bad_timeout_value() {
    assert_parse_error("test_bad_timeout_value", "could not parse integer", 1, true);
}

#[test]
fn parse_error_bad_flashcard_context() {
    assert_parse_error("test_bad_flashcard_context", "expected ]", 1, false);
}

fn assert_parse_error(path: &str, message: &str, lineno: usize, whole_entry: bool) {
    let (_, stderr) = spawn_and_mock(&format!("parse/{}", path), &[], &[]);
    let expected = if whole_entry {
        format!("Error: {} in entry beginning on line {}\n", message, lineno)
    } else {
        format!("Error: {} on line {}\n", message, lineno)
    };
    assert!(stderr == expected, format!("Contents of stderr: {:?}", stderr));
}

fn assert_in_order(mock_stdout: &str, data: &[&str]) {
    let mut last_pos = 0;
    for datum in data {
        if let Some(pos) = mock_stdout[last_pos..].find(datum) {
            // `pos` must be adjusted by an offset of `last_pos` because it is an index
            // in the slice `mock_stdout[last_pos..]` but we want it to be relative to
            // `mock_stdout`.
            last_pos = (pos + last_pos) + datum.len();
        } else {
            panic!("Missing: {:?}; Contents of stdout: {:?}", datum, mock_stdout);
        }
    }
}

fn spawn_and_mock(quiz: &str, input: &[&str], extra_args: &[&str]) -> (String, String) {
    let mut child = spawn(quiz, extra_args);

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        for line in input {
            stdin_write(stdin, &line);
        }
    }

    let result = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
    (stdout, stderr)
}

fn spawn(quiz: &str, extra_args: &[&str]) -> Child {
    Command::new("./target/debug/popquiz")
        .arg("--no-color")
        .arg("-d")
        .arg("./tests/quizzes")
        .arg("take")
        .args(extra_args)
        .arg(&quiz)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child process")
}

fn stdin_write(stdin: &mut ChildStdin, line: &str) {
    stdin.write_all(line.as_bytes()).expect("Failed to write to stdin");
    stdin.write_all("\n".as_bytes()).expect("Failed to write to stdin");
}

fn sleep(millis: u64) {
    thread::sleep(time::Duration::from_millis(millis))
}
