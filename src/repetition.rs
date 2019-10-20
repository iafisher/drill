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
pub fn choose_questions<'a>(questions: &'a Vec<Question>, options: &TakeOptions) -> Vec<&'a Question> {
    let mut candidates = Vec::new();
    for question in questions.iter() {
        if filter_question(question, &options.filter_opts) {
            candidates.push(question);
        }
    }

    let mut buckets = Vec::new();
    for _ in 0..BUCKET_ALLOCATION.len() {
        buckets.push(Vec::new());
    }

    for question in candidates.iter() {
        buckets[get_bucket(question)].push(question);
    }

    let mut rng = thread_rng();
    for bucket in buckets.iter_mut() {
        bucket.shuffle(&mut rng);
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
        chosen.shuffle(&mut rng);
    }

    chosen
}


fn get_bucket(question: &Question) -> usize {
    let mut bucket = 0;
    for result in question.prior_results.iter() {
        // 90% and 40% are arbitrary thresholds that I may need to adjust.
        if result.score >= UP_THRESHOLD && bucket < BUCKET_ALLOCATION.len() - 1 {
            bucket += 1;
        } else if result.score <= DOWN_THRESHOLD  && bucket > 0 {
            bucket -= 1;
        }
    }
    bucket
}


/// Return `true` if `q` satisfies the constraints in `options`.
pub fn filter_question(q: &Question, options: &FilterOptions) -> bool {
    // Either no tags were specified, or `q` has all the specified tags.
    (options.tags.len() == 0 || options.tags.iter().all(|tag| q.tags.contains(tag)))
        // `q` must not have any excluded tags.
        && options.exclude.iter().all(|tag| !q.tags.contains(tag))
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
fn cmp_questions_in_order(a: &&Question, b: &&Question) -> cmp::Ordering {
    if let Some(a_location) = &a.location {
        if let Some(b_location) = &b.location {
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_filter_by_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(s("geography"));

        let mut options = FilterOptions::new();
        assert!(filter_question(&q, &options));

        options.tags.push(s("geography"));
        assert!(filter_question(&q, &options));

        options.tags.push(s("history"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn can_filter_by_excluding_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(s("geography"));

        let mut options = FilterOptions::new();
        options.exclude.push(s("geography"));
        assert!(!filter_question(&q, &options));
    }

    fn s(mystr: &str) -> String {
        String::from(mystr)
    }
}
