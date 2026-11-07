---
name: unit-tests
description: Choose unit tests in this repo by partitioning the input space, and document their coverage in a table in the test file's docstring. Use whenever you write, add, change or review unit tests, or plan which cases to test.
---

# Unit tests

Choose unit tests by partitioning the input space, so that what they cover is deliberate and easy
to check.

1. **Input space:** write down everything the code under test takes as input. For a function that
   moves a bullet, that's the bullet's position and velocity, the black holes and the time step.
2. **Partitionings:** split the input space along a small number of properties at a time. Usually
   this will be along one or two properties at most at a time. Each partitioning is a set of parts
   that don't overlap and together cover the whole space, usually a tree or a grid of conditions.
   For moving a bullet, one partitioning is the number of black holes: none, one, or several.
3. **Coverage:** pick tests (exemplars) so that every part of every partitioning is covered by at
   least one test. One test might cover multiple parts (from multiple partitionings) at once, so
   the minimum number of tests is the largest partitioning's part count, not the size of their
   Cartesian product.
4. **Coverage table:** the test file's docstring lists each partitioning, its parts, and which tests
   cover each part, so a reviewer can check that no part is missing.
5. **One partitioning per PR:** for a large set of tests, add the partitionings one at a time, as
   stacked PRs. Each PR adds or changes tests until its partitioning's parts are all covered.
