#!/bin/sh

rm -rf tests/quizzes/results
cargo test "$@"
status=$?
rm -rf tests/quizzes/results
exit $status
