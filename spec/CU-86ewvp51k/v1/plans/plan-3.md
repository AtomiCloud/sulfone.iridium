# Plan 3: Test Init — Q&A Tree Walking and Test Case Generation

## Goal

Implement `cyanprint test init <path>` which auto-generates template test cases by walking the Q&A tree, exploring all answer combinations (capped), running each combination through template execution, and writing `test.cyan.yaml` + initial snapshots.

## Dependencies

- Plan 1 must be complete (CLI skeleton, config types, template warm-up and execution infrastructure)

## Documentation Requirements

- `init.rs` — doc comments on `run_init`, the tree walker, `ExplorationState`, branching logic, and name generation
- Inline comments on the DFS algorithm explaining the combination cap behavior

## Files to Modify

### `cyanprint/src/test_cmd/init.rs`

Replace the `todo!()` stub with the full implementation.

#### Core Algorithm: Q&A Tree Walker

The tree walker needs to interact with the template server's Q&A endpoint programmatically. `TemplateEngine.start_with()` uses `prompt()` from the `inquire` crate for interactive input — it cannot be used for automated exploration.

**Approach**: Interact with the template server's HTTP API directly via `CyanHttpRepo.prompt_template()`. This gives us:

- `TemplateOutput::QnA(question)` — inspect question, decide answers
- `TemplateOutput::Final(cyan)` — complete path found

This avoids needing to modify `cyanprompt` at all. The `CyanHttpRepo` and `CyanClient` types are already accessible from `cyanprint` (used in `try_cmd.rs` via `run_qa_loop`).

**Key types from cyanprompt** (read-only, no changes needed):

- `CyanHttpRepo` implements `CyanRepo` trait → `prompt_template(TemplateAnswerInput) -> Result<TemplateOutput>`
- `CyanClient` — HTTP client with `endpoint` and `client` fields
- `TemplateAnswerInput` — `{ answers: HashMap<String, Answer>, deterministic_state: HashMap<String, String> }`
- `TemplateOutput::QnA(QnAOutput)` where `QnAOutput` has `.question` (a `Question` enum) and `.deterministic_state`
- `TemplateOutput::Final(FinalOutput)` where `FinalOutput` has `.cyan`
- `Question` enum: `Confirm(ConfirmQuestion)`, `Date(DateQuestion)`, `Checkbox(CheckboxQuestion)`, `Password(PasswordQuestion)`, `Text(TextQuestion)`, `Select(SelectQuestion)`

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

| Question                | Answers to explore                                                                                  |
| ----------------------- | --------------------------------------------------------------------------------------------------- |
| `Question::Text(q)`     | Single: `Answer::String(text_seed)`                                                                 |
| `Question::Password(q)` | Single: `Answer::String(password_seed)`                                                             |
| `Question::Date(q)`     | Single: `Answer::String(date_seed)`                                                                 |
| `Question::Select(q)`   | One per `q.options` entry → `Answer::String(option)` (options are `Vec<String>`)                    |
| `Question::Confirm(q)`  | Two: `Answer::Bool(true)`, `Answer::Bool(false)`                                                    |
| `Question::Checkbox(q)` | Subset: empty + each individual + all → `Answer::StringArray(selected)` (options are `Vec<String>`) |

For non-branching types (Text, Password, Date): add the seed answer to the state, use the seed value as the path label (e.g., `"dummy"`), continue to next question.

For branching types: for each possible answer, clone the state, add the answer, push a label (e.g., option string for Select, `"yes"`/`"no"` for Confirm), and recurse.

**Important**: After receiving a `QnA` response, update `deterministic_state` from `q.deterministic_state` before recursing. The server may update deterministic state at each step.

#### Combination Cap

Use a shared `AtomicUsize` counter. Before forking a new branch, check if counter has reached `max_combinations`. If so, skip new branches (but finish any in-progress DFS path).

#### Name Generation

Concatenate path labels with `-`, sanitize for filesystem (replace non-alphanumeric with `-`, collapse consecutive dashes, lowercase, truncate to 80 chars).

#### Execution Per Combination

Once a `Final` state is reached:

1. Increment the combination counter
2. Record the `answer_state` and `deterministic_state`
3. Run template execution (reuse from Plan 1's template warm-up/per-test logic):
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

The tree walker needs to extract options from Select and Checkbox questions. The `Question` enum variants expose their inner data:

- `SelectQuestion { options: Vec<String>, ... }` — option values are plain strings
- `CheckboxQuestion { options: Vec<String>, ... }` — same
- `ConfirmQuestion` — no options needed, just branch true/false

No code changes needed — just pattern matching on `Question` variants in `init.rs`.

### `cyanprompt/src/domain/services/repo.rs` / `cyanprompt/src/http/client.rs`

The init module needs to call `prompt_template` directly (not through `TemplateEngine`). Check that `CyanHttpRepo` and `CyanClient` are accessible from `cyanprint`. They should be, since `try_cmd.rs` already uses them.

No code changes expected — just ensure the right items are `pub`.

## Approach

1. Study `run_qa_loop()` in `try_cmd.rs` (lines 840-881) to see how `CyanHttpRepo` and `CyanClient` are constructed and used
2. Implement the tree walker with DFS and the combination counter
3. Implement the name generation logic
4. Wire up template execution per combination (reuse Plan 1 infrastructure)
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
- Name collision after truncation → append a counter suffix (e.g., `-1`, `-2`)

## Testing Strategy

- Unit tests for name generation and sanitization
- Unit tests for branching logic with mock question types
- Manual testing against e2e template fixtures to verify tree walking

## Implementation Checklist

- [ ] Implement Q&A tree walker with DFS in `init.rs` (with doc comments)
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
