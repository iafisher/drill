use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time;


#[test]
fn can_take_test1_quiz() {
    let output = spawn_and_mock("test1", &["Ulan Bator", "no"], &[]);
    assert_in_order(
        &output,
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
fn can_take_test2_quiz() {
    let output = spawn_and_mock(
        "test2", &["a", "Wilhelm I", "Wilhelm II", "Wilhelm II"], &["--in-order"],
    );

    assert_in_order(
        &output,
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
    assert!(output.contains("1 incorrect") || output.contains("1 correct"));
}

#[test]
fn can_take_flashcard_quiz() {
    let output = spawn_and_mock(
        "test_flashcard", &["bread", "wine", "butter", "no"], &["--in-order"],
    );

    assert_in_order(
        &output,
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
    let output = spawn_and_mock(
        "test_flashcard",
        &["el pan", "el vino", "la mantequilla", "no"],
        &["--in-order", "--flip"],
    );

    assert_in_order(
        &output,
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
    let output = spawn_and_mock(
        "test_no_credit",
        &["Riverside", "Ontario", "San Bernardino", "Corona", "Fontana", "no"],
        &[],
    );

    assert_in_order(
        &output,
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
    let output = spawn_and_mock(
        "test_instructions",
        &["Lansing, MI", "no"],
        &[],
    );

    assert_in_order(
        &output,
        &[
            "Include the state's postal code.",
            "Correct",
            "100.0%",
        ]
    );
}

#[test]
fn timeouts_work() {
    // This test can't use `spawn_and_mock` because it needs to control how long the
    // thread sleeps between answering questions.
    let mut process = spawn("test_timeouts", &[]);
    let stdin = process.stdin.as_mut().expect("Failed to open stdin");
    stdin_write(stdin, "Chisinau");
    sleep(1500);
    stdin_write(stdin, "Kiev");

    let result = process.wait_with_output().expect("Failed to read stdout");
    let output = String::from_utf8_lossy(&result.stdout).to_string();

    assert_in_order(
        &output,
        &[
            "Warning: This quiz contains timed questions!",
            "Correct!\n",
            "Correct, but you exceeded the time limit",
        ],
    );
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

fn spawn_and_mock(quiz: &str, input: &[&str], extra_args: &[&str]) -> String {
    let mut child = spawn(quiz, extra_args);

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        for line in input {
            stdin_write(stdin, &line);
        }
    }

    let result = child.wait_with_output().expect("Failed to read stdout");
    String::from_utf8_lossy(&result.stdout).to_string()
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
