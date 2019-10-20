/**
 * Choose the most optimal questions.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::cmp::Ordering;

use rand::seq::SliceRandom;
use rand::thread_rng;

use super::common::{FilterOptions, TakeOptions};
use super::quiz::{Question, QuestionResult};


/// Choose a set of questions, filtered by the command-line options.
pub fn choose_questions<'a>(questions: &'a Vec<Question>, options: &TakeOptions) -> Vec<&'a Question> {
    let mut candidates = Vec::new();
    for question in questions.iter() {
        if filter_question(question, &options.filter_opts) {
            candidates.push(question);
        }
    }

    // --best and --worst can only be applied to questions with at least one
    // scored response, so we remove questions with no scored responses here.
    if options.best.is_some() || options.worst.is_some() {
        let mut i = 0;
        while i < candidates.len() {
            if aggregate_results(&candidates[i].prior_results).is_none() {
                candidates.remove(i);
            } else {
                i += 1;
            }
        }
    }

    if let Some(best) = options.best {
        candidates.sort_by(cmp_questions_best);
        candidates.truncate(best);
    } else if let Some(worst) = options.worst {
        candidates.sort_by(cmp_questions_worst);
        candidates.truncate(worst);
    }

    if let Some(most) = options.most {
        candidates.sort_by(cmp_questions_most);
        candidates.truncate(most);
    } else if let Some(least) = options.least {
        candidates.sort_by(cmp_questions_least);
        candidates.truncate(least);
    }

    if !options.in_order {
        let mut rng = thread_rng();
        candidates.shuffle(&mut rng);
    }

    // Important that this operation comes after the --most and --least flags have
    // been applied, e.g. if --most 50 -n 10 we want to choose 10 questions among
    // the 50 most asked, not the most asked among 10 random questions.
    //
    // Also important that this occurs after shuffling.
    if let Some(num_to_ask) = options.num_to_ask {
        candidates.truncate(num_to_ask);
    }

    candidates
}


/// Return `true` if `q` satisfies the constraints in `options`.
pub fn filter_question(q: &Question, options: &FilterOptions) -> bool {
    // Either no tags were specified, or `q` has all the specified tags.
    (options.tags.len() == 0 || options.tags.iter().all(|tag| q.tags.contains(tag)))
        // `q` must not have any excluded tags.
        && options.exclude.iter().all(|tag| !q.tags.contains(tag))
        // If `--never` flag is present, question must not have been asked before.
        && (!options.never || q.prior_results.len() == 0)
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


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the highest previous scores come first.
fn cmp_questions_best(a: &&Question, b: &&Question) -> Ordering {
    let a_score = aggregate_results(&a.prior_results).unwrap_or(0.0);
    let b_score = aggregate_results(&b.prior_results).unwrap_or(0.0);

    if a_score > b_score {
        Ordering::Less
    } else if a_score < b_score {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the lowest previous scores come first.
fn cmp_questions_worst(a: &&Question, b: &&Question) -> Ordering {
    return cmp_questions_best(a, b).reverse();
}


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the most responses come first.
fn cmp_questions_most(a: &&Question, b: &&Question) -> Ordering {
    let a_results = a.prior_results.len();
    let b_results = b.prior_results.len();

    if a_results > b_results {
        Ordering::Less
    } else if a_results < b_results {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the least responses come first.
fn cmp_questions_least(a: &&Question, b: &&Question) -> Ordering {
    return cmp_questions_most(a, b).reverse();
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
