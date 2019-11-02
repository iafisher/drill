#!/bin/sh

rm -rf tests/quizzes/results
mkdir -p tests/quizzes/long/results
rm -rf tests/quizzes/long/results/*
cp tests/quizzes/long/persistent-results/* tests/quizzes/long/results
cargo test "$@"
rm -rf tests/quizzes/results
rm -rf tests/quizzes/long/results/*
