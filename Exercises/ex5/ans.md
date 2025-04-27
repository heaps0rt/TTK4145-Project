### Part 5: Reflecting

You do not have to answer every question in turn, as long as you address the contents somewhere.

- Condition variables, Java monitors, and Ada protected objects are quite similar in what they do (temporarily yield execution so some other task can unblock us).
  - But in what ways do these mechanisms differ?
    - Condition variables: Requires mutex pairing and manual condition checks with wait loop.
    - Java monitors: Implicit monitor generated through 'synchronized', but no native support for priority.
    - Ada protected objects: Guarded entries automatically queue tasks. Handles the complexities internally, and guarantees atomicity.

- Bugs in this kind of low-level synchronization can be hard to spot.
  - Which solutions are you most confident are correct?
    - Ada protected objects and Go request message passing.
  - Why, and what does this say about code quality?
    - Ada's guards are evaluated atomically, and Go's message passing has no shared variables, only a centralized resource manager.
    - Code quality improves with higher-level abstractions

- We operated only with two priority levels here, but it makes sense for this "kind" of priority resource to support more priorities.
  - How would you extend these solutions to N priorities? Is it even possible to do this elegantly?
    - Semaphores: Add a semaphore for each priority. Generalizable, but with an extra iteration for each priority the deallocate can be bogged down.
    - Condition variables: Already generalized, very elegant solution.
    - Protected objects: Add new entries for each priority level. Scales linearly, but for large values of N this will be a lot of code, so hard to generalize.
    - Message passing request: Built-in N priorities, elegant solution.
    - Message passing priority select: For priority select a new channel must be established for each priority level, introducing more complexity to the select workaround.
  - What (if anything) does that say about code quality?
    - no

- In D's standard library, `getValue` for semaphores is not even exposed (probably because it is not portable – Windows semaphores don't have `getValue`, though you could hack it together with `ReleaseSemaphore()` and `WaitForSingleObject()`).
  - A leading question: Is using `getValue` ever appropriate?
    - No
  - Explain your intuition: What is it that makes `getValue` so dubious?
    - no

- Which one(s) of these different mechanisms do you prefer, both for this specific task and in general? (This is a matter of taste – there are no "right" answers here)
    - no