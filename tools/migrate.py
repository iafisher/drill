#!/usr/bin/env python3
"""
Use this script to migrate quizzes from the old JSON format of version 1 to the textual
format of version 2.

Note that ungraded questions and dependencies are no longer supported in version 2. Some
niche use cases (like having "q" as the first side of a flashcard) may no longer work.

Author:  Ian Fisher (iafisher@protonmail.com)
Version: September 2019
"""
import argparse
import json
import os
import subprocess
import sys
from collections import OrderedDict


def main():
    parser = argparse.ArgumentParser(
        description="Migrate quizzes from old JSON format to new text format."
    )
    parser.add_argument("name")
    parser.add_argument(
        "-f", "--force", action="store_true", help="Overwrite old quiz."
    )
    parser.add_argument(
        "--stdout", action="store_true", help="Print v2 of quiz to standard output."
    )
    args = parser.parse_args()

    path = get_path(args.name)
    if path is None:
        warning(
            "No such quiz named '{}'; assuming that it is a file path instead.",
            args.name
        )
        path = args.name
    else:
        info("Detected quiz at {}.", path)

    try:
        with open(path, "r", encoding="utf-8") as f:
            quiz = json.load(f, object_hook=OrderedDict)
    except FileNotFoundError:
        error("Could not open '{}' for reading.", path)
    except IOError:
        error("Could not read from '{}' due to IO error.", path)

    if args.stdout:
        try:
            migrate(quiz, sys.stdout)
        except RuntimeError as e:
            error(str(e))
    else:
        pathname, ext = os.path.splitext(path)
        new_path = pathname + "-v2" + ext
        if os.path.exists(new_path):
            if not args.force:
                error(
                    "Quiz named '{}-v2' already exists. Use -f to overwrite.", args.name
                )
            else:
                info("Overwriting quiz at '{}' due to -f flag.", new_path)
        else:
            info("Creating new quiz at '{}'.", new_path)

        try:
            with open(new_path, "w") as writer:
                migrate(quiz, writer)
        except FileNotFoundError:
            error("Could not open '{}' for writing.", path)
        except IOError:
            error("Could not write to '{}' due to IO error.", path)
        except RuntimeError as e:
            error(str(e))

    if EXPLANATIONS:
        warning("'explanations' field is no longer supported.")


def get_path(name):
    env = os.environ.copy()
    env["NO_COLOR"] = "yes"
    proc = subprocess.run(
        ["quiz", "path", name],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=env,
    )
    path = proc.stdout.strip().decode("utf-8")
    if path.startswith(("Error", "error")):
        return None
    else:
        return path


EXPLANATIONS = False
def migrate(quiz, writer):
    default_kind = quiz.get("default_kind", "ShortAnswer")
    for i, question in enumerate(quiz["questions"]):
        writer.write("[{}] ".format(i+1))
        kind = question.get("kind", default_kind)
        if "text" in question:
            if isinstance(question["text"], str):
                writer.write(question["text"])
            else:
                writer.write(question["text"][0])
            writer.write("\n")
        elif kind != "Flashcard":
            raise RuntimeError("question missing `text` field")

        if kind == "ShortAnswer":
            write_answer(question["answer"], writer)
        elif kind == "ListAnswer" or kind == "OrderedListAnswer":
            for answer in question["answer_list"]:
                write_answer(answer, writer)
            if kind == "OrderedListAnswer":
                writer.write("- ordered: true\n")
        elif kind == "MultipleChoice":
            write_answer(question["answer"], writer)
            writer.write("- choices:  ")
            write_answer(question["candidates"], writer)
        elif kind == "Ungraded":
            raise RuntimeError("Ungraded questions are no longer supported")
        elif kind == "Flashcard":
            writer.write(question["side1"])
            writer.write(" = ")
            write_answer(question["side2"], writer)
        else:
            raise RuntimeError("unknown kind: " + kind)

        if "explanations" in question:
            EXPLANATIONS = True

        if "tags" in question:
            writer.write("- tags: ")
            writer.write(", ".join(question["tags"]))
            writer.write("\n")

        writer.write("\n")


def write_answer(answer, writer):
    if isinstance(answer, str):
        writer.write(answer)
    else:
        writer.write(" / ".join(answer))
    writer.write("\n")


def escape_answer(answer):
    return answer.replace("/", "\\/").replace(",", "\\,")


def info(msg, *args):
    print("\033[36m[INFO]\033[0m " + msg.format(*args))


def warning(msg, *args):
    print("\033[33m[WARNING]\033[0m " + msg.format(*args))


def error(msg, *args, exit_code=2):
    sys.stderr.write("\033[31m[ERROR]\033[0m " + msg.format(*args) + "\n")
    if exit_code is not None:
        sys.exit(exit_code)


if __name__ == "__main__":
    main()
