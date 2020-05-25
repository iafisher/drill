#!/bin/sh

rm -rf tests/quizzes/results
export DRILL_HOME=$(dirname $0)
cargo test "$@"
status=$?
rm -rf tests/quizzes/results
exit $status
