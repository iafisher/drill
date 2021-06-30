/**
 * Choose the most optimal questions to ask based on past results.
 *
 * Bucket 0: never asked before
 * Bucket 1: don't know at all, should ask immediately
 * Bucket 2: just learned, should ask within a day
 * Bucket 3: should ask within a week
 * Bucket 4: ask once a month or so
 *
 * All questions in Bucket 0 will be asked, and the remaining number of questions will
 * consist roughly of 50% questions from Bucket 1, 20% questions each from Bucket 2 and
 * Bucket 3, and 10% questions from Bucket 4.
 *
 * Author:  Ian Fisher (iafisher@fastmail.com)
 * Version: October 2019
 */
use std::cmp;

use rand::seq::SliceRandom;
use rand::thread_rng;

use super::common;
use super::common::TakeOptions;
use super::quiz2::{Question2, QuestionResult2};

// The percentage of questions that come from each bucket, expressed as integer
// fractions, e.g. 2 means 1/2, 5 means 1/5 etc.
const BUCKET_ALLOCATION: [usize; 5] = [1, 2, 5, 5, 10];
// What percentage correct for a question to move up a bucket.
const UP_THRESHOLD: u64 = 900;
// What percentage correct for a question to move down a bucket.
const DOWN_THRESHOLD: u64 = 400;

/// Choose a set of questions, filtered by the command-line options.
pub fn choose_questions<'a>(
    questions: &'a Vec<Question2>,
    options: &TakeOptions,
) -> Vec<&'a Question2> {
    let mut candidates = Vec::new();
    // TODO(2021-06-30): Filter by command-line options.
    for question in questions.iter() {
        candidates.push(question);
    }

    // TODO(2021-06-30): Port over v1 algorithm for spaced repetition.
    let mut rng = thread_rng();
    candidates.shuffle(&mut rng);
    candidates.truncate(options.num_to_ask);
    candidates
}
