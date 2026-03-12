# Plan 3: Test Init — Q&A Tree Walking and Test Case Generation

## Goal

Implement `cyanprint test init <path>` which auto-generates template test cases by walking the Q&A tree, exploring all answer combinations (capped), running each combination through template execution, and writing `test.cyan.yaml` + initial snapshots.

## Dependencies

- Plan 1 must be complete (CLI skeleton, config types)
- Plan 2 must be complete (template warm-up and execution infrastructure)

## Files to Modify

### `cyanprint/src/test_cmd/init.rs`

Replace the `todo!()` stub with the full implementation.

#### Core Algorithm: Q&A Tree Walker

The tree walker needs to interact with the template server's Q&A endpoint programmatically. Currently `TemplateEngine.start_with()` uses `prompt()` from the `inquire` crate for interactive input. For init, we need a non-interactive variant that can:

1. Call the template server with current answers
2. Inspect the returned question type
3. Decide which answer(s) to try (based on question type and seed values)
4. Fork state for branching questions

**Approach**: Don't use `TemplateEngine.start_with()` directly. Instead, interact with the template server's HTTP API at the same level the engine does — via `CyanHttpRepo.prompt_template()`. This gives us:

- `TemplateOutput::QnA(question)` — inspect question, decide answers
- `TemplateOutput::Final(cyan)` — complete path found

This avoids needing to modify `cyanprompt` at all.

#### Tree Walk State

```
struct ExplorationState {
    answers: HashMap<String, Answer>,
    deterministic_state: HashMap<String, String>,
    path_labels: Vec<String>,  // for name generation
}
```

#### Branching Logic

At each `QnA` response, inspect the question variant:

| Question      | Answers to explore                                                     |
| ------------- | ---------------------------------------------------------------------- |
| `Text(q)`     | Single: `Answer::String(text_seed)`                                    |
| `Password(q)` | Single: `Answer::String(password_seed)`                                |
| `Date(q)`     | Single: `Answer::String(date_seed)`                                    |
| `Select(q)`   | One per `q.options` → `Answer::String(option.value)`                   |
| `Confirm(q)`  | Two: `Answer::Bool(true)`, `Answer::Bool(false)`                       |
| `Checkbox(q)` | Subset: empty + each individual + all. `Answer::StringArray(selected)` |

For non-branching types (Text, Password, Date): add the seed answer to the state, use the seed value as the path label (e.g., `"dummy"`), continue to next question.

For branching types: for each possible answer, clone the state, add the answer, push a label (e.g., option name for Select, `"yes"`/`"no"` for Confirm), and recurse.

#### Combination Cap

Use a shared `AtomicUsize` counter. Before forking a new branch, check if counter has reached `max_combinations`. If so, skip new branches (but finish any in-progress DFS path).

#### Name Generation

Concatenate path labels with `-`, sanitize for filesystem (replace non-alphanumeric with `-`, collapse consecutive dashes, lowercase, truncate to 80 chars).

#### Execution Per Combination

Once a `Final` state is reached:

1. Increment the combination counter
2. Record the `answer_state` and `deterministic_state`
3. Run template execution (reuse from Plan 2's template warm-up/per-test logic):
   - Generate session_id, merger_id
   - `try_setup` → `bootstrap` → execute → unpack to `{output}/{test_name}/`
   - Session cleanup
4. Copy output from `{output}/{test_name}/` to `fixtures/expected/{test_name}/`

#### Final Output

1. Write `test.cyan.yaml` with all generated test cases:
   ```yaml
   tests:
     - name: {generated_name}
       expected: ./fixtures/expected/{generated_name}
       answer_state:
         {question_id}: { type: String, value: "..." }
         ...
       deterministic_state:
         {key}: "{value}"
         ...
   ```
2. Create `fixtures/expected/{test_name}/` directories (already populated)
3. Cleanup: remove `{output}/` tmp directory
4. Cleanup: stop template container, remove images, remove blob

### `cyanprompt/src/domain/models/question.rs`

The tree walker needs to extract options from Select and Checkbox questions. Check if the `Question` enum variants expose their inner data (options list, etc.). The `QuestionTrait` trait provides `.id()`. We may need to pattern match on variants to access options.

No code changes expected here — just reading. The init module will pattern match on `Question` variants directly.

### `cyanprompt/src/domain/services/repo.rs` / `cyanprompt/src/http/client.rs`

The init module needs to call `prompt_template` directly (not through `TemplateEngine`). Check that `CyanHttpRepo` and `CyanClient` are accessible from `cyanprint`. They should be, since `try_cmd.rs` already uses them.

No code changes expected — just ensure the right items are `pub`.

## Approach

1. Study `TemplateEngine.start_with()` and `CyanHttpRepo.prompt_template()` to understand the exact request/response flow
2. Implement the tree walker with DFS and the combination counter
3. Implement the name generation logic
4. Wire up template execution per combination (reuse Plan 2 infrastructure)
5. Implement `test.cyan.yaml` writer (serialize `TestConfig` to YAML)
6. Wire up in main.rs `Commands::Test::Init` handler
7. Test with a real template (e.g., e2e fixtures)

## Edge Cases

- Template with no branching questions (all Text) → generates exactly 1 test case
- Template with many Select options → cap kicks in, only first N combinations explored (DFS order)
- Checkbox with 0 options → skip (no branching)
- Q&A server returns error → fail init with descriptive message
- `test.cyan.yaml` already exists → warn and ask for confirmation, or use `--force` to overwrite
- `fixtures/expected/` already has content → same, warn or overwrite
- Very deep Q&A tree (many questions) → long names get truncated

## Testing Strategy

- Unit tests for name generation and sanitization
- Unit tests for branching logic with mock question types
- Manual testing against e2e template fixtures to verify tree walking

## Implementation Checklist

- [ ] Implement Q&A tree walker with DFS in `init.rs`
- [ ] Implement branching logic for all question types
- [ ] Implement combination cap with AtomicUsize counter
- [ ] Implement name generation and sanitization
- [ ] Implement per-combination template execution
- [ ] Implement `test.cyan.yaml` serialization/writing
- [ ] Implement `fixtures/expected/` directory creation
- [ ] Wire up `Commands::Test::Init` in main.rs
- [ ] Add tmp cleanup after init
- [ ] Handle existing `test.cyan.yaml` / fixtures (warn or overwrite)
- [ ] Add unit tests for name generation
- [ ] Test with e2e template fixtures
