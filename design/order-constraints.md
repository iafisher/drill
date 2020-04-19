# Order Constraints in Drill

Document status: Early draft

Author: Ian Fisher

Last updated: 2020-01-06



I would like to be able to constrain the possible order of questions while keeping the order mostly random. How can I do it?

Idea

- Add an `- after: <id>` attribute

- Shuffling algorithm

  - Iterate over the list removing any question that (a) has an after attribute or (b) is named by an after attribute.

    - One pass to remove all questions with an after attribute.
    - Another pass to remove all questions named by an after attribute (from a question in the current list).

  - Shuffle the original list.

  - Order the removed questions and insert them at random points.

- Shuffling algorithm 2

  - Shuffle the list of questions.
  - Iterate over the shuffled list in two passes noting any question that (a) has an after attribute or (b) is named by an after attribute of another question.
  - Sort these questions in-place in the list using topological sort.

- Remaining questions

  - How do I insert *n* items into a list at random points?

  - Do I (a) analyze the graph of the entire quiz to deterministically detect cycles, or (b) only detect cycles when they appear in the selected questions, thereby sometimes failing and sometimes succeeding when there is an error?

    - Maybe I should just forbid chains

      - I probably want to allow one question to be the target of multiple after attributes though, e.g. a question defining a term which multiple other questions then use.

Idea

- Define ordering relationships at the quiz level.