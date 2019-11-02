#!/bin/sh

rm -rf tests/quizzes/results
cargo test "$@"
rm -rf tests/quizzes/results
