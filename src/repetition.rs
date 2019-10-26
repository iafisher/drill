/**
 * Choose the most optimal questions to ask based on past results.
 *
 * Bucket 1: don't know at all, should ask immediately
 * Bucket 2: just learned, should ask within a day
 * Bucket 3: should ask within a week
 * Bucket 4: ask once a month or so
 *
 * Each quiz will consist roughly of 50% questions from Bucket 1, 20% questions each
 * from Bucket 2 and Bucket 3, and 10% questions from Bucket 4.
 *
 * TODO: Take time asked into account.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::cmp;

use rand::seq::SliceRandom;
use rand::thread_rng;

use super::common::{FilterOptions, TakeOptions};
use super::quiz::{Question, QuestionResult};


// The percentage of questions that come from each bucket, expressed as integer
// fractions, e.g. 2 means 1/2, 5 means 1/5 etc.
const BUCKET_ALLOCATION: [usize; 4] = [2, 5, 5, 10];
// What percentage correct for a question to move up a bucket.
const UP_THRESHOLD: f64 = 0.9;
// What percentage correct for a question to move down a bucket.
const DOWN_THRESHOLD: f64 = 0.4;


/// Choose a set of questions, filtered by the command-line options.
pub fn choose_questions<'a>(questions: &'a Vec<Box<Question>>, options: &TakeOptions) -> Vec<&'a Box<Question>> {
    let mut candidates = Vec::new();
    for question in questions.iter() {
        if filter_tags(&question.get_common().tags, &options.filter_opts) {
            candidates.push(question);
        }
    }

    let mut buckets = Vec::new();
    for _ in 0..BUCKET_ALLOCATION.len() {
        buckets.push(Vec::new());
    }

    for question in candidates.iter() {
        buckets[get_bucket(&question.get_common().prior_results)].push(question);
    }

    for bucket in buckets.iter_mut() {
        bucket.sort_by(cmp_questions_oldest_first);
    }

    let mut chosen = Vec::new();
    let mut cumulative_allocation = 0;
    for i in 0..BUCKET_ALLOCATION.len() {
        let mut allocation = options.num_to_ask / BUCKET_ALLOCATION[i];
        if i == BUCKET_ALLOCATION.len() - 1 {
            allocation = options.num_to_ask - chosen.len();
        } else {
            // If previous buckets didn't have enough questions to fill their
            // allocations, spill over the extra question allocation into this bucket.
            allocation += cumulative_allocation - chosen.len();
        }
        allocation = cmp::min(allocation, buckets[i].len());
        for j in 0..allocation {
            chosen.push(*buckets[i][j]);
        }
        cumulative_allocation += allocation;
    }

    if options.in_order {
        chosen.sort_by(cmp_questions_in_order);
    } else {
        let mut rng = thread_rng();
        chosen.shuffle(&mut rng);
    }

    chosen
}


fn get_bucket(results: &Vec<QuestionResult>) -> usize {
    let mut bucket = 0;
    for result in results.iter() {
        // 90% and 40% are arbitrary thresholds that I may need to adjust.
        if result.score >= UP_THRESHOLD && bucket < BUCKET_ALLOCATION.len() - 1 {
            bucket += 1;
        } else if result.score <= DOWN_THRESHOLD  && bucket > 0 {
            bucket -= 1;
        }
    }
    bucket
}


/// Return `true` if `tags` satisfies the constraints in `options`.
pub fn filter_tags(tags: &Vec<String>, options: &FilterOptions) -> bool {
    // Either no tags were specified, or `q` has all the specified tags.
    (options.tags.len() == 0 || options.tags.iter().all(|tag| tags.contains(tag)))
        // `q` must not have any excluded tags.
        && options.exclude.iter().all(|tag| !tags.contains(tag))
}


/// Return the percentage of correct responses in the vector of results. `None` is
/// returned when the vector is empty.
pub fn aggregate_results(results: &Vec<QuestionResult>) -> Option<f64> {
    let mut sum = 0.0;
    let mut graded_count = 0;
    for result in results.iter() {
        sum += result.score;
        graded_count += 1;
    }

    if graded_count > 0 {
        Some(100.0 * (sum / (graded_count as f64)))
    } else {
        None
    }
}


/// Comparison function that sorts an array of `Question` objects in the order the
/// questions appeared in the original quiz file based on the `location` field.
fn cmp_questions_in_order(a: &&Box<Question>, b: &&Box<Question>) -> cmp::Ordering {
    let a_common = a.get_common();
    let b_common = b.get_common();
    if let Some(a_location) = &a_common.location {
        if let Some(b_location) = &b_common.location {
            if a_location.line < b_location.line {
                cmp::Ordering::Less
            } else if a_location.line > b_location.line {
                cmp::Ordering::Greater
            } else {
                // This case should never happen because two questions can't be defined
                // on the same line.
                cmp::Ordering::Equal
            }
        } else {
            cmp::Ordering::Greater
        }
    } else {
        cmp::Ordering::Less
    }
}


/// Comparison function that sorts an array of `Question` objects so that the questions
/// that were least recently asked appear first. Questions that have never been asked
/// will appear at the very front.
fn cmp_questions_oldest_first(a: &&&Box<Question>, b: &&&Box<Question>) -> cmp::Ordering {
    // NOTE: This method assumes that the `prior_results` field of `Question` objects
    // is ordered chronologically, which should always be true.
    let a_common = a.get_common();
    let b_common = b.get_common();
    if a_common.prior_results.len() > 0 {
        if b_common.prior_results.len() > 0 {
            let a_last = a_common.prior_results.last().unwrap().time_asked;
            let b_last = b_common.prior_results.last().unwrap().time_asked;
            a_last.partial_cmp(&b_last).unwrap_or(cmp::Ordering::Equal)
        } else {
            cmp::Ordering::Greater
        }
    } else {
        if b_common.prior_results.len() > 0 {
            cmp::Ordering::Less
        } else {
            cmp::Ordering::Equal
        }
    }
}
