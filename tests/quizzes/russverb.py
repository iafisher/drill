#!/usr/bin/env python3
import random
import sys


def main():
    text = sys.argv[1]
    assert sys.argv[2] == ""

    english, russian = text.split("=", maxsplit=1)
    english = english.strip()
    variants = russian.split("/")
    aspect_pairs = [v.split(",") for v in variants]

    if random.randint(0, 1) == 0:
        impf = [pair[0].strip() for pair in aspect_pairs]
        print(english + " = " + " / ".join(impf))
    else:
        perf = [pair[1].strip() for pair in aspect_pairs]
        print(english + " [perf] = " + " / ".join(perf))


if __name__ == "__main__":
    main()
