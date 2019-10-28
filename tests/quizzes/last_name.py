#!/usr/bin/env python3
import sys

text = sys.argv[1]
answer = sys.argv[2]
first, last = answer.split()

print(text + " (changed)")
print(answer + " / " + last)
