use std::io::Write;
use std::process::{Command, Stdio};


#[test]
fn can_take_test1_quiz() {
    let output = spawn_and_mock(".test1-v2", &["Ulan Bator", "no"], &[]);
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
        ".test2-v2", &["a", "Wilhelm I", "Wilhelm II", "Wilhelm II"], &["--in-order"],
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
        ".test_flashcard-v2", &["bread", "wine", "butter", "no"], &["--in-order"],
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
        ".test_flashcard-v2",
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

fn assert_in_order(mock_stdout: &str, data: &[&str]) {
    let mut last_pos = 0;
    let mut in_order = true;
    for datum in data {
        if let Some(pos) = mock_stdout.find(datum) {
            if pos < last_pos {
                in_order = false;
                break;
            } else {
                last_pos = pos;
            }
        } else {
            in_order = false;
            break;
        }
    }
    assert!(in_order, "Contents of stdout: {:?}", mock_stdout);
}

fn spawn_and_mock(quiz: &str, input: &[&str], extra_args: &[&str]) -> String {
    let mut child = Command::new("./target/debug/popquiz")
        .arg("take")
        .arg("--no-color")
        .args(extra_args)
        .arg(&quiz)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child process");

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        for line in input {
            stdin.write_all(line.as_bytes()).expect("Failed to write to stdin");
            stdin.write_all("\n".as_bytes()).expect("Failed to write to stdin");
        }
    }

    let result = child.wait_with_output().expect("Failed to read stdout");
    String::from_utf8_lossy(&result.stdout).to_string()
}
