use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time;

use regex::Regex;


#[test]
fn can_take_simple_quiz1() {
    play_quiz(
        "test1",
        &[],
        &[
            "(1) What is the capital of Mongolia?",
            "> Ulan Bator",
            "Correct!",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}


#[test]
fn can_take_simple_quiz2() {
    play_quiz(
        "test2",
        &["--in-order"],
        &[
            "(1) Who was President of the United States during the Korean War?",
            r"RE: \(a\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"RE: \(b\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"RE: \(c\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"RE: \(d\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"> Harry Truman",
            "Please enter a letter.",
            r"> a",
            // Since the order of the choices is random, guessing 'a' may or may not
            // have been correct.
            r"RE: (Correct!|Incorrect\. The correct answer was Harry S\. Truman\.)",
            "(2) List the modern Emperors of Germany in chronological order.",
            "> Wilhelm I",
            "Correct!",
            "> Wilhelm II",
            "Incorrect. The correct answer was Frederick III.",
            "> Wilhelm II",
            "Correct!",
            "Score for this question: 66.6%",
            r"RE: Score: (33\.3|83\.3)% out of 2 questions",
            r"RE: (0|1) correct",
            "1 partially correct",
            r"RE: (0|1) incorrect",
        ],
    );
}

#[test]
fn can_take_quiz_with_list_question() {
    play_quiz(
        "test_list",
        &["--in-order"],
        &[
            "(1) Name the five members of the UN Security Council.",
            "> China",
            "Correct!",
            "> PR China",
            "You already said that.",
            "> France",
            "Correct!",
            "> Germany",
            "Incorrect.",
            "> US",
            "Correct!",
            "> United Kingdom",
            "Correct!",
            "You missed:",
            "Russia",
            "Score for this question: 80.0%",
            "Score: 80.0% out of 1 question",
            "0 correct",
            "1 partially correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_take_flashcard_quiz() {
    play_quiz(
        "test_flashcard",
        &["--in-order"],
        &[
            "(1) el pan",
            "> bread",
            "Correct!",
            "(2) el vino",
            "> wine",
            "Correct!",
            "(3) la mantequilla",
            "> butter",
            "Correct!",
            "Score: 100.0% out of 3 questions",
            "3 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_take_flipped_flashcard_quiz() {
    play_quiz(
        "test_flashcard",
        &["--in-order", "--flip"],
        &[
            "(1) bread",
            "> el pan",
            "Correct!",
            "(2) wine",
            "> el vino",
            "Correct!",
            "(3) butter",
            "> la mantequilla",
            "Correct!",
            "Score: 100.0% out of 3 questions",
            "3 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn no_credit_answers_work() {
    play_quiz(
        "test_no_credit",
        &[],
        &[
            "(1) Name the three largest cities of the Inland Empire.",
            "> Riverside",
            "Correct!",
            "> Ontario",
            "No credit.",
            "> San Bernardino",
            "Correct!",
            "> Corona",
            "No credit.",
            "> Fontana",
            "Correct!",
            "Score for this question: 100.0%",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn quiz_instructions_are_displayed() {
    play_quiz(
        "test_instructions",
        &[],
        &[
            "Include the state's postal code.",
            "(1) What is the capital of Michigan?",
            "> Lansing, MI",
            "Correct!",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn flashcards_context() {
    play_quiz(
        "test_flashcard_context",
        &[],
        &[
            "(1) to read [perf]",
            "> прочитать",
            "Correct!",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );

    play_quiz(
        "test_flashcard_context",
        &["--flip"],
        &[
            "(1) прочитать [bleh]",
            "> to read",
            "Correct!",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn timeouts_work() {
    // This test can't use `play_quiz` because it needs to control how long the thread
    // sleeps between answering questions.
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
            "Correct!\n",
            "exceeded time limit",
            "Correct!\n",
            "Correct!\n",
            // Make sure we got full credit for the list question.
            "2 correct",
            "1 partially correct",
        ],
    );
}

#[test]
fn can_correct_questions_in_quiz() {
    play_quiz(
        "test_correction",
        &["--in-order"],
        &[
            "(1) What is the largest city in Northern California?",
            "> San Jose",
            "Incorrect. The correct answer was San Francisco.",
            "(2) What is the largest city in Oregon?",
            "> !!",
            "Previous answer marked correct.",
            "(2) What is the largest city in Oregon?",
            "> Eugene",
            "Incorrect. The correct answer was Portland.",
            "(3) Name two things.",
            "> foo",
            "Correct!",
            "> !!",
            "Previous answer marked correct.",
            "(3) Name two things.",
            "> foo",
            "Correct!",
            "> bar",
            "Correct!",
            "Score for this question: 100.0%",
            "Score: 100.0% out of 3 questions",
            "3 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn unicode_normalization_works() {
    play_quiz(
        "test_unicode_normalization",
        &[],
        &[
            "(1) traffic",
            "> el tra\u{0301}fico",
            "Correct!",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_use_custom_script() {
    play_quiz(
        "test_custom_script",
        &[],
        &[
            "(1) Who was the first President of the United States? (changed)",
            "> Washington",
            "Correct!",
            "Score: 100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_use_global_custom_script() {
    play_quiz(
        "test_global_custom_script",
        &["--in-order"],
        &[
            "(1) Who was the first President of the United States? (changed)",
            "> Washington",
            "Correct!",
            "(2) Who was the second President of the United States? (changed)",
            "> John Adams",
            "Correct!",
            "Score: 100.0% out of 2 questions",
            "2 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn parse_error_no_blank_line_between_questions() {
    assert_parse_error(
        "test_no_blank_line", "no blank line between questions", 2, false);
}

#[test]
fn parse_error_no_blank_line_after_settings() {
    assert_parse_error(
        "test_no_blank_line_after_settings",
        "no blank line after quiz settings",
        2,
        false,
    );
}

#[test]
fn parse_error_wrong_ordered_value() {
    assert_parse_error(
        "test_wrong_ordered_value",
        "ordered field must be either 'true' or 'false'",
        1,
        true,
    );
}

#[test]
fn parse_error_no_first_line() {
    assert_parse_error(
        "test_no_first_line", "expected first line of question", 1, false);
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

#[test]
fn parse_error_unknown_field() {
    assert_parse_error("test_unknown_field", "unexpected field 'whatever'", 1, true);
}

#[test]
fn parse_error_unknown_global_field() {
    assert_parse_error(
        "test_unknown_global_field", "unexpected field 'whatever'", 1, false);
}

#[test]
fn parse_error_field_on_wrong_question() {
    assert_parse_error(
        "test_field_on_wrong_question", "unexpected field 'nocredit'", 1, true);
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

fn play_quiz(name: &str, extra_args: &[&str], in_out: &[&str]) {
    let mut child = spawn(name, extra_args);
    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        for line in in_out {
            if line.starts_with("> ") {
                stdin_write(stdin, &line[1..]);
            }
        }
    }

    let result = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();

    let mut lines_iter = stdout.lines();
    for expected in in_out {
        if !expected.starts_with("> ") {
            let mut got = lines_iter.next().expect("Premature end of output");
            loop {
                if got.trim().len() == 0 {
                    got = lines_iter.next().expect("Premature end of output");
                } else {
                    break;
                }
            }

            if expected.starts_with("RE:") {
                let re = Regex::new(&expected[3..].trim()).unwrap();
                assert!(
                    re.is_match(&got.trim()),
                    format!(
                        "Failed to match {:?} against pattern {:?}",
                        got.trim(),
                        &expected[3..],
                    )
                );
            } else {
                assert!(
                    expected.trim() == got.trim(),
                    format!("Expected {:?}, got {:?}", expected.trim(), got.trim()),
                );
            }
        }
    }

    while let Some(line) = lines_iter.next() {
        if line.trim().len() > 0 {
            panic!("Extra output: {:?}", line.trim());
        }
    }
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
    Command::new("./target/debug/drill")
        .arg("--no-color")
        .arg("take")
        .args(extra_args)
        .arg(&format!("tests/quizzes/{}", quiz))
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
