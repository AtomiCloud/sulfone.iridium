//! Test initialization with Q&A tree walking and snapshot generation.
//!
//! This module implements `cyanprint test init <path>` which:
//! - Walks the Q&A tree of a template using DFS
//! - Explores all answer combinations (capped by max_combinations)
//! - Writes `test.cyan.yaml` with generated test cases
//! - Runs `test template --update-snapshots` to generate initial snapshots

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use inquire::{Confirm, DateSelect, MultiSelect, Text};

use bollard::Docker;
use reqwest::blocking::Client;

use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::question::{Question, QuestionTrait};
use cyanprompt::domain::models::template::{input::TemplateAnswerInput, output::TemplateOutput};
use cyanprompt::domain::services::repo::{CyanHttpRepo, CyanRepo};
use cyanprompt::http::client::CyanClient;
use cyanregistry::cli::mapper::read_build_config;
use cyanregistry::cli::models::template_config::CyanTemplateFileConfig;
use cyanregistry::http::client::CyanRegistryClient;

use crate::docker::buildx::BuildxBuilder;
use crate::port::find_available_port;
use crate::test_cmd::config::{AnswerStateEntry, ExpectedOutput, TestCase, TestConfig};
use crate::test_cmd::template::run_template_tests;
use crate::try_cmd::{ensure_daemon_running, pre_flight_validation};

/// Type alias for branch configurations to avoid type complexity warnings.
type BranchConfig = HashMap<String, Vec<(Answer, String)>>;

/// Exploration state for Q&A tree walking.
///
/// Contains the current state while traversing the Q&A tree:
/// - `answers`: Collected answers keyed by question ID
/// - `deterministic_state`: Server-maintained state that affects template behavior
/// - `path_labels`: Labels for each question answered, used for test name generation
#[derive(Debug, Clone)]
struct ExplorationState {
    answers: HashMap<String, Answer>,
    deterministic_state: HashMap<String, String>,
    path_labels: Vec<String>,
}

impl ExplorationState {
    fn new() -> Self {
        Self {
            answers: HashMap::new(),
            deterministic_state: HashMap::new(),
            path_labels: Vec::new(),
        }
    }

    fn add_answer(&mut self, question_id: String, answer: Answer, label: String) {
        self.answers.insert(question_id, answer);
        self.path_labels.push(label);
    }

    fn update_deterministic_state(&mut self, state: HashMap<String, String>) {
        self.deterministic_state.extend(state);
    }
}

/// Classification of a question type for discovery purposes.
///
/// Mirrors `Question` variants but is `Clone`-able and stores only type + message,
/// since `Question` itself doesn't derive `Clone`.
#[derive(Debug, Clone, PartialEq)]
enum DiscoveredQuestionType {
    Text,
    Password,
    Date,
    Select,
    Confirm,
    Checkbox,
}

/// A question discovered during pass-1 tree walking.
#[derive(Debug, Clone)]
struct DiscoveredQuestion {
    id: String,
    question_type: DiscoveredQuestionType,
    message: String,
    branches: Vec<(Answer, String)>, // (answer_value, display_label)
}

/// Extract the question type and message from a `Question` reference.
fn classify_question(question: &Question) -> (DiscoveredQuestionType, String) {
    match question {
        Question::Text(q) => (DiscoveredQuestionType::Text, q.message.clone()),
        Question::Password(q) => (DiscoveredQuestionType::Password, q.message.clone()),
        Question::Date(q) => (DiscoveredQuestionType::Date, q.message.clone()),
        Question::Select(q) => (DiscoveredQuestionType::Select, q.message.clone()),
        Question::Confirm(q) => (DiscoveredQuestionType::Confirm, q.message.clone()),
        Question::Checkbox(q) => (DiscoveredQuestionType::Checkbox, q.message.clone()),
    }
}

/// Generated test case result from Q&A tree exploration.
#[derive(Debug, Clone)]
struct GeneratedTestCase {
    name: String,
    answer_state: HashMap<String, AnswerStateEntry>,
    deterministic_state: HashMap<String, String>,
}

/// Initialize test configuration and generate snapshots.
///
/// This function:
/// 1. Warms up a template container (for Q&A walking only)
/// 2. Walks the Q&A tree via DFS to discover all answer combinations
/// 3. Writes `test.cyan.yaml` with generated test cases
/// 4. Runs `run_template_tests` with `update_snapshots=true` to generate initial snapshots
///
/// # Branching Logic
///
/// | Question       | Answers explored                                   |
/// | -------------- | -------------------------------------------------- |
/// | Text           | Single: seed value (default "dummy")              |
/// | Password       | Single: seed value (default "password123")        |
/// | Date           | Single: seed value (default "2024-01-01")         |
/// | Select         | One per option in q.options                        |
/// | Confirm        | Two: true, false                                   |
/// | Checkbox       | Subset: empty + each individual + all combinations |
#[allow(clippy::too_many_arguments)]
pub fn run_init(
    path: &str,
    max_combinations: Option<usize>,
    text_seed: Option<&str>,
    password_seed: Option<&str>,
    date_seed: Option<&str>,
    parallel: usize,
    interactive: bool,
    output: &str,
    config: &str,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    registry_client: &CyanRegistryClient,
) -> Result<(), Box<dyn Error + Send>> {
    println!("Initializing test configuration for template at: {path}");
    let max_combinations = max_combinations.unwrap_or(30);
    println!("Max combinations: {max_combinations}");

    // Back up existing test config and snapshots before overwriting
    let backup_dir = backup_existing_artifacts(path)?;

    // Phase 1: Walk Q&A tree to generate test.cyan.yaml
    println!("\nWarming up template for Q&A walking...");
    let warmup = qa_warmup(
        path,
        config,
        coordinator_endpoint,
        disable_daemon_autostart,
        registry_client,
    )?;

    let walk_result = walk_and_write_config(
        &warmup,
        path,
        max_combinations,
        text_seed,
        password_seed,
        date_seed,
        interactive,
    );

    // Always clean up warmup resources
    println!("\nCleaning up Q&A walking resources...");
    let _ = cleanup_qa_warmup(&warmup);

    // Propagate walk errors after cleanup
    walk_result?;

    // Phase 2: Run template tests with --update-snapshots to generate initial snapshots
    println!("\nGenerating initial snapshots by running tests with --update-snapshots...");
    let results = run_template_tests(
        path,
        None, // no filter — run all generated tests
        parallel,
        true, // update_snapshots = true
        config,
        output,
        None, // no junit output
        coordinator_endpoint,
        disable_daemon_autostart,
        registry_client,
    )?;

    // Report results (suppress failures since this is init — snapshots are being created)
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();
    println!("\nInitialization complete!");
    println!("  Test cases generated: {}", results.len());
    if failed > 0 {
        println!("  Snapshots created: {passed} (with {failed} failures — review manually)");
    } else {
        println!("  All snapshots created successfully");
    }
    println!(
        "  Configuration: {}",
        PathBuf::from(path).join("test.cyan.yaml").display()
    );
    println!(
        "  Fixtures: {}",
        PathBuf::from(path)
            .join("fixtures")
            .join("expected")
            .display()
    );

    // Ask user about backup cleanup
    if let Some(ref backup) = backup_dir {
        println!("\n  Backup saved at: {}", backup.display());
        let delete = Confirm::new("Delete backup?")
            .with_default(false)
            .prompt()
            .unwrap_or(false);
        if delete {
            if let Err(e) = fs::remove_dir_all(backup) {
                eprintln!("  Warning: failed to remove backup: {e}");
            } else {
                println!("  Backup deleted");
            }
        } else {
            println!("  Backup kept at: {}", backup.display());
        }
    }

    Ok(())
}

/// Back up existing test.cyan.yaml and fixtures/expected before init overwrites them.
///
/// Returns the backup directory path if anything was backed up, or None if nothing existed.
fn backup_existing_artifacts(path: &str) -> Result<Option<PathBuf>, Box<dyn Error + Send>> {
    let base = PathBuf::from(path);
    let test_config = base.join("test.cyan.yaml");
    let fixtures_dir = base.join("fixtures").join("expected");

    if !test_config.exists() && !fixtures_dir.exists() {
        return Ok(None);
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let backup_dir = base.join(format!(".cyan_backup_{timestamp}"));
    fs::create_dir_all(&backup_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create backup directory: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    if test_config.exists() {
        let dest = backup_dir.join("test.cyan.yaml");
        fs::copy(&test_config, &dest).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to back up test.cyan.yaml: {e}"
            ))) as Box<dyn Error + Send>
        })?;
        println!("  Backed up test.cyan.yaml");
    }

    if fixtures_dir.exists() {
        let dest = backup_dir.join("fixtures").join("expected");
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to create backup fixtures dir: {e}"
            ))) as Box<dyn Error + Send>
        })?;
        copy_dir_all(&fixtures_dir, &dest)?;
        println!("  Backed up fixtures/expected/");
    }

    println!("  Backup saved to: {}", backup_dir.display());

    // Remove originals after backup so init starts from a clean slate
    if test_config.exists() {
        fs::remove_file(&test_config).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to remove old test.cyan.yaml: {e}"
            ))) as Box<dyn Error + Send>
        })?;
        println!("  Removed old test.cyan.yaml");
    }

    if fixtures_dir.exists() {
        fs::remove_dir_all(&fixtures_dir).map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Failed to remove old fixtures/expected: {e}"
            ))) as Box<dyn Error + Send>
        })?;
        println!("  Removed old fixtures/expected/");
    }

    Ok(Some(backup_dir))
}

/// Copy a directory recursively.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), Box<dyn Error + Send>> {
    fs::create_dir_all(dst).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    for entry in fs::read_dir(src).map_err(|e| Box::new(e) as Box<dyn Error + Send>)? {
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        }
    }
    Ok(())
}

/// Walk Q&A tree and write test.cyan.yaml.
fn walk_and_write_config(
    warmup: &QaWarmup,
    path: &str,
    max_combinations: usize,
    text_seed: Option<&str>,
    password_seed: Option<&str>,
    date_seed: Option<&str>,
    interactive: bool,
) -> Result<(), Box<dyn Error + Send>> {
    let test_config_path = PathBuf::from(path).join("test.cyan.yaml");

    // Create fixtures directory
    let fixtures_dir = PathBuf::from(path).join("fixtures").join("expected");
    fs::create_dir_all(&fixtures_dir).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to create fixtures directory {}: {e}",
            fixtures_dir.display()
        ))) as Box<dyn Error + Send>
    })?;

    // Walk Q&A tree
    println!("\nWalking Q&A tree to generate test cases...");
    let generated_tests = walk_qa_tree(
        warmup,
        max_combinations,
        text_seed.unwrap_or("dummy"),
        password_seed.unwrap_or("password123"),
        date_seed.unwrap_or("2024-01-01"),
        interactive,
    )?;

    println!("\nGenerated {} test case(s)", generated_tests.len());

    // Write test.cyan.yaml
    println!("Writing test.cyan.yaml...");
    write_test_config(&test_config_path, &generated_tests)?;
    println!("test.cyan.yaml written successfully");

    Ok(())
}

/// Walk the Q&A tree using a two-pass approach to generate test cases.
///
/// Pass 1 (`discovery_dfs`): Discover all unique questions and their branches.
/// Uses a visited set so each question ID is only branched on once, avoiding
/// combinatorial explosion during discovery.
///
/// Pass 2 (`full_dfs`): Full combinatorial DFS with per-path state, using the
/// branch configuration from pass 1 (optionally modified by the user in
/// interactive mode).
fn walk_qa_tree(
    warmup: &QaWarmup,
    max_combinations: usize,
    text_seed: &str,
    password_seed: &str,
    date_seed: &str,
    interactive: bool,
) -> Result<Vec<GeneratedTestCase>, Box<dyn Error + Send>> {
    let template_endpoint = format!("http://localhost:{}", warmup.port);
    let http_client = Rc::new(
        Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?,
    );

    let repo = CyanHttpRepo {
        client: CyanClient {
            endpoint: template_endpoint,
            client: http_client,
        },
    };

    // Pass 1: Discover all unique questions and their branches
    println!("  Pass 1: Discovering questions...");
    let mut visited = HashSet::new();
    let mut discovered: Vec<DiscoveredQuestion> = Vec::new();
    let initial_state = ExplorationState::new();

    discovery_dfs(
        &repo,
        initial_state,
        &mut visited,
        &mut discovered,
        text_seed,
        password_seed,
        date_seed,
    )?;

    println!("  Discovered {} unique question(s)", discovered.len());

    // Build branch_config from discovered questions (or let user modify)
    let branch_config: HashMap<String, Vec<(Answer, String)>> =
        if interactive && !discovered.is_empty() {
            println!("\n  Select which branches to keep/expand per question.\n");
            interactive_modify(&discovered)?
        } else {
            discovered
                .iter()
                .map(|dq| (dq.id.clone(), dq.branches.clone()))
                .collect()
        };

    // Pass 2: Full combinatorial DFS using branch_config
    println!("  Pass 2: Generating test cases...");
    let combination_counter = AtomicUsize::new(0);
    let mut generated_tests = Vec::new();
    let mut used_names = HashMap::new();

    let initial_state = ExplorationState::new();
    full_dfs(
        &repo,
        initial_state,
        &branch_config,
        &mut generated_tests,
        &mut used_names,
        &combination_counter,
        max_combinations,
        text_seed,
        password_seed,
        date_seed,
    )?;

    Ok(generated_tests)
}

/// Display string for an `AnswerStateEntry`.
#[allow(dead_code)]
fn answer_display(entry: &AnswerStateEntry) -> String {
    match entry {
        AnswerStateEntry::String(s) => s.clone(),
        AnswerStateEntry::StringArray(arr) => {
            if arr.is_empty() {
                "none".to_string()
            } else {
                arr.join("+")
            }
        }
        AnswerStateEntry::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
    }
}

/// Pass 1: Discovery DFS — find all unique questions and their branches.
///
/// Uses a `visited` set so each question ID is only fully branched on once.
/// When a question ID is encountered again (on a different tree path), it reuses
/// the first branch's answer to continue deeper without branching, avoiding
/// combinatorial explosion during discovery.
fn discovery_dfs(
    repo: &CyanHttpRepo,
    state: ExplorationState,
    visited: &mut HashSet<String>,
    discovered: &mut Vec<DiscoveredQuestion>,
    text_seed: &str,
    password_seed: &str,
    date_seed: &str,
) -> Result<(), Box<dyn Error + Send>> {
    let input = TemplateAnswerInput {
        answers: state.answers.clone(),
        deterministic_state: state.deterministic_state.clone(),
    };

    let output = repo.prompt_template(input)?;

    match output {
        TemplateOutput::Final(_) => {
            // Base case — leaf node, nothing more to discover
            Ok(())
        }
        TemplateOutput::QnA(qna) => {
            let mut updated_state = state.clone();
            updated_state.update_deterministic_state(qna.deterministic_state);

            let question_id = qna.question.id();
            let branches = get_answer_branches(&qna.question, text_seed, password_seed, date_seed);

            if visited.insert(question_id.clone()) {
                // First time seeing this question — record it and explore all branches
                let (question_type, message) = classify_question(&qna.question);
                discovered.push(DiscoveredQuestion {
                    id: question_id.clone(),
                    question_type,
                    message,
                    branches: branches.clone(),
                });

                for (answer, label) in &branches {
                    let mut branch_state = updated_state.clone();
                    branch_state.add_answer(question_id.clone(), answer.clone(), label.clone());
                    discovery_dfs(
                        repo,
                        branch_state,
                        visited,
                        discovered,
                        text_seed,
                        password_seed,
                        date_seed,
                    )?;
                }
            } else {
                // Already visited — reuse first branch answer to continue deeper
                if let Some((answer, label)) = branches.into_iter().next() {
                    let mut branch_state = updated_state;
                    branch_state.add_answer(question_id, answer, label);
                    discovery_dfs(
                        repo,
                        branch_state,
                        visited,
                        discovered,
                        text_seed,
                        password_seed,
                        date_seed,
                    )?;
                }
            }

            Ok(())
        }
    }
}

/// Returns `true` if a branch with the given label already exists.
fn branch_already_exists(branches: &[(Answer, String)], label: &str) -> bool {
    branches.iter().any(|(_, l)| l == label)
}

/// Convert an `inquire::InquireError` into our boxed error type.
fn inquire_err(e: inquire::InquireError) -> Box<dyn Error + Send> {
    Box::new(std::io::Error::other(e.to_string()))
}

/// Interactive modification of discovered branches.
///
/// - **Text/Password**: Reprompt loop — shows default seed, asks for values one at a time
///   until user says no to "Add another?". Deduplicates by label.
/// - **Date**: `DateSelect` date-picker widget; reprompt loop with dedup on formatted date.
/// - **Checkbox**: `MultiSelect` from individual options per combination; dedup on sorted label.
/// - **Select/Confirm**: `MultiSelect` to de-select branches; enforces ≥1 branch via reprompt.
fn interactive_modify(
    discovered: &[DiscoveredQuestion],
) -> Result<BranchConfig, Box<dyn Error + Send>> {
    let mut branch_config: HashMap<String, Vec<(Answer, String)>> = HashMap::new();

    for dq in discovered {
        match dq.question_type {
            DiscoveredQuestionType::Text | DiscoveredQuestionType::Password => {
                if dq.branches.is_empty() {
                    branch_config.insert(dq.id.clone(), dq.branches.clone());
                    continue;
                }

                let default_seed = &dq.branches[0].1;
                println!(
                    "\n  {} [{}] (default: '{}')",
                    dq.id, dq.message, default_seed
                );

                let mut branches: Vec<(Answer, String)> = Vec::new();

                // Prompt for first value
                let first = Text::new(&format!("  Enter value (empty to keep '{default_seed}'):"))
                    .prompt()
                    .map_err(inquire_err)?;

                let first_trimmed = first.trim();
                if first_trimmed.is_empty() {
                    branches.push(dq.branches[0].clone());
                } else if !branch_already_exists(&branches, first_trimmed) {
                    branches.push((
                        Answer::String(first_trimmed.to_string()),
                        first_trimmed.to_string(),
                    ));
                } else {
                    println!("  (skipped duplicate '{first_trimmed}')");
                }

                // Reprompt loop
                loop {
                    let add_more = Confirm::new("  Add another value?")
                        .with_default(false)
                        .prompt()
                        .map_err(inquire_err)?;

                    if !add_more {
                        break;
                    }

                    let next = Text::new("  Enter value:").prompt().map_err(inquire_err)?;

                    let next_trimmed = next.trim();
                    if !next_trimmed.is_empty() {
                        if branch_already_exists(&branches, next_trimmed) {
                            println!("  (skipped duplicate '{next_trimmed}')");
                        } else {
                            branches.push((
                                Answer::String(next_trimmed.to_string()),
                                next_trimmed.to_string(),
                            ));
                        }
                    }
                }

                println!("  {}: {} value(s)", dq.id, branches.len());
                branch_config.insert(dq.id.clone(), branches);
            }
            DiscoveredQuestionType::Date => {
                if dq.branches.is_empty() {
                    branch_config.insert(dq.id.clone(), dq.branches.clone());
                    continue;
                }

                let default_seed = &dq.branches[0].1;
                println!(
                    "\n  {} [{}] (default: '{}')",
                    dq.id, dq.message, default_seed
                );

                let mut branches: Vec<(Answer, String)> = Vec::new();

                // Prompt for first date using DateSelect widget
                let first_date =
                    DateSelect::new(&format!("  Pick a date (default: '{default_seed}'):"))
                        .prompt()
                        .map_err(inquire_err)?;

                let label = first_date.format("%Y-%m-%d").to_string();
                if !branch_already_exists(&branches, &label) {
                    branches.push((Answer::String(label.clone()), label));
                } else {
                    println!("  (skipped duplicate '{label}')");
                }

                // Reprompt loop for more dates
                loop {
                    let add_more = Confirm::new("  Add another date?")
                        .with_default(false)
                        .prompt()
                        .map_err(inquire_err)?;

                    if !add_more {
                        break;
                    }

                    let next_date = DateSelect::new("  Pick date:")
                        .prompt()
                        .map_err(inquire_err)?;

                    let label = next_date.format("%Y-%m-%d").to_string();
                    if branch_already_exists(&branches, &label) {
                        println!("  (skipped duplicate '{label}')");
                    } else {
                        branches.push((Answer::String(label.clone()), label));
                    }
                }

                println!("  {}: {} date(s)", dq.id, branches.len());
                branch_config.insert(dq.id.clone(), branches);
            }
            DiscoveredQuestionType::Checkbox => {
                if dq.branches.is_empty() {
                    branch_config.insert(dq.id.clone(), dq.branches.clone());
                    continue;
                }

                // Extract individual option names from discovered branches
                let available_options: Vec<String> = dq
                    .branches
                    .iter()
                    .filter_map(|(answer, _)| match answer {
                        Answer::StringArray(arr) if arr.len() == 1 => Some(arr[0].clone()),
                        _ => None,
                    })
                    .collect();

                println!(
                    "\n  {} [{}] — options: [{}]",
                    dq.id,
                    dq.message,
                    available_options.join(", ")
                );
                println!("  Select combinations one at a time using the multi-select picker.");

                let mut branches: Vec<(Answer, String)> = Vec::new();

                // First combination via MultiSelect
                let first_selected = MultiSelect::new(
                    "  Select options for this combination (none = empty set):",
                    available_options.clone(),
                )
                .prompt()
                .map_err(inquire_err)?;

                let mut sorted = first_selected;
                sorted.sort();
                let label = if sorted.is_empty() {
                    "none".to_string()
                } else {
                    sorted.join("+")
                };
                if !branch_already_exists(&branches, &label) {
                    branches.push((Answer::StringArray(sorted), label));
                } else {
                    println!("  (skipped duplicate '{label}')");
                }

                // Reprompt loop for more combinations
                loop {
                    let add_more = Confirm::new("  Add another combination?")
                        .with_default(false)
                        .prompt()
                        .map_err(inquire_err)?;

                    if !add_more {
                        break;
                    }

                    let next_selected = MultiSelect::new(
                        "  Select options for this combination (none = empty set):",
                        available_options.clone(),
                    )
                    .prompt()
                    .map_err(inquire_err)?;

                    let mut sorted = next_selected;
                    sorted.sort();
                    let label = if sorted.is_empty() {
                        "none".to_string()
                    } else {
                        sorted.join("+")
                    };
                    if branch_already_exists(&branches, &label) {
                        println!("  (skipped duplicate '{label}')");
                    } else {
                        branches.push((Answer::StringArray(sorted), label));
                    }
                }

                println!("  {}: {} combination(s)", dq.id, branches.len());
                branch_config.insert(dq.id.clone(), branches);
            }
            DiscoveredQuestionType::Select | DiscoveredQuestionType::Confirm => {
                if dq.branches.len() <= 1 {
                    if !dq.branches.is_empty() {
                        println!(
                            "  {}: {} (single value, auto-included)",
                            dq.id, dq.branches[0].1
                        );
                    }
                    branch_config.insert(dq.id.clone(), dq.branches.clone());
                    continue;
                }

                // MultiSelect to de-select branches — reprompt until ≥1 selected
                let labels: Vec<String> = dq.branches.iter().map(|(_, l)| l.clone()).collect();
                let indices: Vec<usize> = (0..labels.len()).collect();

                let selected = loop {
                    let result = MultiSelect::new(
                        &format!("{} [{}] — select branches to keep:", dq.id, dq.message),
                        labels.clone(),
                    )
                    .with_default(&indices)
                    .prompt()
                    .map_err(inquire_err)?;

                    if result.is_empty() {
                        println!("  At least 1 branch is required. Please select again.");
                        continue;
                    }
                    break result;
                };

                let kept: Vec<(Answer, String)> = dq
                    .branches
                    .iter()
                    .filter(|(_, label)| selected.contains(label))
                    .cloned()
                    .collect();
                println!(
                    "  {}: keeping {} of {} branches",
                    dq.id,
                    kept.len(),
                    dq.branches.len()
                );
                branch_config.insert(dq.id.clone(), kept);
            }
        }
    }

    // Safety net: if any question ended up with 0 branches after user interaction,
    // fall back to the discovered defaults.
    for dq in discovered {
        if let Some(branches) = branch_config.get(&dq.id) {
            if branches.is_empty() && !dq.branches.is_empty() {
                println!(
                    "  Warning: {} had 0 branches after modification, restoring defaults",
                    dq.id
                );
                branch_config.insert(dq.id.clone(), dq.branches.clone());
            }
        }
    }

    Ok(branch_config)
}

/// Pass 2: Full combinatorial DFS with per-path state.
///
/// Uses `branch_config` to determine which branches to explore for each question.
/// If a question ID is not found in `branch_config` (e.g. it was only reachable
/// via a different deterministic_state in pass 1), falls back to
/// `get_answer_branches()` with default seeds.
#[allow(clippy::too_many_arguments)]
fn full_dfs(
    repo: &CyanHttpRepo,
    state: ExplorationState,
    branch_config: &HashMap<String, Vec<(Answer, String)>>,
    generated_tests: &mut Vec<GeneratedTestCase>,
    used_names: &mut HashMap<String, usize>,
    combination_counter: &AtomicUsize,
    max_combinations: usize,
    text_seed: &str,
    password_seed: &str,
    date_seed: &str,
) -> Result<(), Box<dyn Error + Send>> {
    if combination_counter.load(Ordering::Relaxed) >= max_combinations {
        return Ok(());
    }

    let input = TemplateAnswerInput {
        answers: state.answers.clone(),
        deterministic_state: state.deterministic_state.clone(),
    };

    let output = repo.prompt_template(input)?;

    match output {
        TemplateOutput::Final(_final_output) => {
            let combination_id = combination_counter.fetch_add(1, Ordering::Relaxed);

            if combination_id >= max_combinations {
                return Ok(());
            }

            println!("  [{}] Final state reached", combination_id + 1);

            let name = generate_test_name(&state.path_labels, combination_id, used_names);

            // Convert answers to AnswerStateEntry format
            let mut answer_state = HashMap::new();
            for (question_id, answer) in &state.answers {
                let entry = match answer {
                    Answer::String(s) => AnswerStateEntry::String(s.clone()),
                    Answer::StringArray(arr) => AnswerStateEntry::StringArray(arr.clone()),
                    Answer::Bool(b) => AnswerStateEntry::Bool(*b),
                };
                answer_state.insert(question_id.clone(), entry);
            }

            generated_tests.push(GeneratedTestCase {
                name: name.clone(),
                answer_state,
                deterministic_state: state.deterministic_state.clone(),
            });

            println!("    Generated test case: {name}");
        }
        TemplateOutput::QnA(qna) => {
            let mut updated_state = state.clone();
            updated_state.update_deterministic_state(qna.deterministic_state);

            let question_id = qna.question.id();

            // Use branch_config if available, otherwise fall back to default branches
            let branches = if let Some(configured) = branch_config.get(&question_id) {
                configured.clone()
            } else {
                get_answer_branches(&qna.question, text_seed, password_seed, date_seed)
            };

            for (answer, label) in branches {
                if combination_counter.load(Ordering::Relaxed) >= max_combinations {
                    break;
                }

                let mut branch_state = updated_state.clone();
                branch_state.add_answer(question_id.clone(), answer, label);

                full_dfs(
                    repo,
                    branch_state,
                    branch_config,
                    generated_tests,
                    used_names,
                    combination_counter,
                    max_combinations,
                    text_seed,
                    password_seed,
                    date_seed,
                )?;
            }
        }
    }

    Ok(())
}

/// Get answer branches for a question.
fn get_answer_branches(
    question: &Question,
    text_seed: &str,
    password_seed: &str,
    date_seed: &str,
) -> Vec<(Answer, String)> {
    match question {
        Question::Text(_q) => {
            vec![(Answer::String(text_seed.to_string()), text_seed.to_string())]
        }
        Question::Password(_q) => {
            vec![(
                Answer::String(password_seed.to_string()),
                password_seed.to_string(),
            )]
        }
        Question::Date(_q) => {
            vec![(Answer::String(date_seed.to_string()), date_seed.to_string())]
        }
        Question::Select(q) => q
            .options
            .iter()
            .map(|opt| (Answer::String(opt.clone()), opt.clone()))
            .collect(),
        Question::Confirm(_q) => {
            vec![
                (Answer::Bool(true), "yes".to_string()),
                (Answer::Bool(false), "no".to_string()),
            ]
        }
        Question::Checkbox(q) => {
            if q.options.is_empty() {
                return Vec::new();
            }

            let mut branches = Vec::new();

            // Empty selection
            branches.push((Answer::StringArray(Vec::new()), "none".to_string()));

            // Each individual option
            for opt in &q.options {
                branches.push((Answer::StringArray(vec![opt.clone()]), opt.clone()));
            }

            // All options (only when there are at least 2, to avoid duplicating the singleton)
            if q.options.len() > 1 {
                branches.push((Answer::StringArray(q.options.clone()), "all".to_string()));
            }

            branches
        }
    }
}

/// Generate a test name from path labels.
///
/// Labels are joined with `:` to separate answers. Within each label,
/// non-alphanumeric characters become `-`, so `"my project"` → `"my-project"`
/// and two answers `"opt1"` + `"opt2"` → `"opt1:opt2"`.
fn generate_test_name(
    path_labels: &[String],
    combination_id: usize,
    used_names: &mut HashMap<String, usize>,
) -> String {
    let base_name = if path_labels.is_empty() {
        format!("test{combination_id}")
    } else {
        // Sanitize each label individually, then join with ':'
        let sanitized_labels: Vec<String> = path_labels
            .iter()
            .map(|label| {
                let sanitized = label
                    .chars()
                    .map(|c| if c.is_alphanumeric() { c } else { '-' })
                    .collect::<String>();
                // Collapse consecutive dashes within each label
                sanitized
                    .split('-')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("-")
            })
            .collect();
        sanitized_labels.join(":").to_lowercase()
    };

    // Compute truncated name first (without suffix adjustment) to key deduplication
    let base_truncated_prelim = if base_name.len() > 80 {
        truncated_with_ellipsis(&base_name, 80)
    } else {
        base_name.clone()
    };

    let count = used_names.entry(base_truncated_prelim.clone()).or_insert(0);

    let suffix_len = if *count > 0 {
        count.to_string().len() + 1
    } else {
        0
    };
    let max_base_len = 80 - suffix_len;

    let base_truncated = if base_name.len() > max_base_len {
        truncated_with_ellipsis(&base_name, max_base_len)
    } else {
        base_name
    };

    let final_name = if *count > 0 {
        let name_with_suffix = format!("{base_truncated}-{count}");
        *count += 1;
        name_with_suffix
    } else {
        *count += 1;
        base_truncated
    };

    if final_name.len() > 80 {
        truncated_with_ellipsis(&final_name, 80)
    } else {
        final_name
    }
}

fn truncated_with_ellipsis(s: &str, max_len: usize) -> String {
    if max_len <= 3 {
        return "...".to_string();
    }
    if s.len() <= max_len {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_len - 3).collect();
    format!("{truncated}...")
}

/// Write test configuration to test.cyan.yaml.
fn write_test_config(
    config_path: &Path,
    generated_tests: &[GeneratedTestCase],
) -> Result<(), Box<dyn Error + Send>> {
    let test_cases: Vec<TestCase> = generated_tests
        .iter()
        .map(|gt| {
            let relative_path = PathBuf::from("fixtures")
                .join("expected")
                .join(&gt.name)
                .to_string_lossy()
                .to_string();

            TestCase {
                name: gt.name.clone(),
                expected: ExpectedOutput::Snapshot {
                    path: relative_path,
                },
                answer_state: gt.answer_state.clone(),
                deterministic_state: gt.deterministic_state.clone(),
                validate: Vec::new(),
                input: None,
                globs: None,
                config: None,
                resolver_inputs: None,
            }
        })
        .collect();

    let config = TestConfig { tests: test_cases };

    let yaml_str = serde_yaml::to_string(&config).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to serialize YAML: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    fs::write(config_path, yaml_str).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to write test.cyan.yaml: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    Ok(())
}

// --- Lightweight Q&A warmup (template container only, no coordinator session) ---

/// Minimal warmup context for Q&A tree walking.
/// Only needs the template container running — no coordinator session required.
struct QaWarmup {
    container_name: String,
    port: u16,
    template_image_ref: String,
    blob_image_ref: String,
    docker: Docker,
}

/// Start a template container for Q&A walking.
///
/// This is a lightweight warmup that only starts the template container.
/// No coordinator session is created — that happens later when `run_template_tests` executes.
fn qa_warmup(
    template_path: &str,
    config_path: &str,
    coordinator_endpoint: &str,
    disable_daemon_autostart: bool,
    registry_client: &CyanRegistryClient,
) -> Result<QaWarmup, Box<dyn Error + Send>> {
    let full_config_path = PathBuf::from(template_path).join(config_path);
    let content = fs::read_to_string(full_config_path).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to read config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    let template_config: CyanTemplateFileConfig = serde_yaml::from_str(&content).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to parse config file: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    // Pre-flight validation
    println!("Running pre-flight validation...");
    pre_flight_validation(template_path, false)?;

    // Ensure daemon is running
    println!("Ensuring daemon is running...");
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    ensure_daemon_running(&docker, disable_daemon_autostart, coordinator_endpoint)?;

    // Resolve dependencies (needed to build synthetic template for image building)
    println!("Resolving and pinning dependencies...");
    let pinned = crate::try_cmd::resolve_and_pin_dependencies(registry_client, &template_config)?;

    // Build images
    println!("Building template and blob images...");
    let build_config_path = PathBuf::from(template_path).join(config_path);
    let build_config = read_build_config(build_config_path.to_string_lossy().to_string())?;

    let registry = build_config.registry.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No registry configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let images = build_config.images.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No images configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let (blob_docker_ref, template_docker_ref) =
        build_template_images(registry, images, template_path)?;

    println!("Images built successfully");

    // Create synthetic template (needed for container setup)
    let local_template_id = uuid::Uuid::new_v4().to_string();
    let build_result = Some((
        Some(blob_docker_ref.clone()),
        Some(template_docker_ref.clone()),
    ));

    let _template = crate::try_cmd::build_synthetic_template(
        &local_template_id,
        &template_config,
        &pinned,
        false,
        build_result.as_ref(),
    )?;

    // Start template container for Q&A walking
    println!("Starting template container...");
    let port = find_available_port(5600, 5900).ok_or_else(|| {
        Box::new(std::io::Error::other(
            "No available port in range 5600-5900",
        )) as Box<dyn Error + Send>
    })?;

    let container_name = format!("cyan-template-{}", local_template_id.replace('-', ""));

    crate::try_cmd::start_template_container(
        &docker,
        &container_name,
        &template_docker_ref,
        port,
        coordinator_endpoint,
        "cyanprint.test",
        None,
    )?;

    println!("Template container started on port {port}");

    // Health check
    println!("Health checking template container...");
    crate::try_cmd::health_check_template_container(port, 30, 2)?;

    Ok(QaWarmup {
        container_name,
        port,
        template_image_ref: template_docker_ref,
        blob_image_ref: blob_docker_ref,
        docker,
    })
}

/// Clean up Q&A warmup resources (container + images).
fn cleanup_qa_warmup(warmup: &QaWarmup) -> Result<(), Box<dyn Error + Send>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    println!("  Removing container: {}", warmup.container_name);
    runtime.block_on(async {
        let _ = warmup
            .docker
            .stop_container(&warmup.container_name, None)
            .await;
        warmup
            .docker
            .remove_container(&warmup.container_name, None)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    })?;

    println!("  Removing template image: {}", warmup.template_image_ref);
    runtime.block_on(async {
        warmup
            .docker
            .remove_image(
                &warmup.template_image_ref,
                None::<bollard::query_parameters::RemoveImageOptions>,
                None::<bollard::auth::DockerCredentials>,
            )
            .await
            .map(|_| ())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    })?;

    println!("  Removing blob image: {}", warmup.blob_image_ref);
    runtime.block_on(async {
        warmup
            .docker
            .remove_image(
                &warmup.blob_image_ref,
                None::<bollard::query_parameters::RemoveImageOptions>,
                None::<bollard::auth::DockerCredentials>,
            )
            .await
            .map(|_| ())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    })?;

    Ok(())
}

/// Build template and blob images.
fn build_template_images(
    registry: &str,
    images: &cyanregistry::cli::models::build_config::ImagesConfig,
    template_path: &str,
) -> Result<(String, String), Box<dyn Error + Send>> {
    let template_path_abs = PathBuf::from(template_path);

    let blob_config = images.blob.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No blob image configured")) as Box<dyn Error + Send>
    })?;

    let blob_name = blob_config.image.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(
            "blob image name not specified in build config",
        )) as Box<dyn Error + Send>
    })?;

    println!("  Building blob image...");
    let blob_dockerfile_path = template_path_abs.join(&blob_config.dockerfile);
    let blob_context_path = template_path_abs.join(&blob_config.context);

    crate::try_cmd::build_image(
        &BuildxBuilder::new(),
        registry,
        blob_name,
        "latest",
        blob_dockerfile_path.to_string_lossy().as_ref(),
        blob_context_path.to_string_lossy().as_ref(),
        &[],
    )?;

    let blob_ref = format!("{registry}/{blob_name}:latest");

    let template_config = images.template.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No template image configured")) as Box<dyn Error + Send>
    })?;

    let template_name = template_config.image.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(
            "template image name not specified in build config",
        )) as Box<dyn Error + Send>
    })?;

    println!("  Building template image...");
    let template_dockerfile_path = template_path_abs.join(&template_config.dockerfile);
    let template_context_path = template_path_abs.join(&template_config.context);

    crate::try_cmd::build_image(
        &BuildxBuilder::new(),
        registry,
        template_name,
        "latest",
        template_dockerfile_path.to_string_lossy().as_ref(),
        template_context_path.to_string_lossy().as_ref(),
        &[],
    )?;

    let template_ref = format!("{registry}/{template_name}:latest");

    Ok((blob_ref, template_ref))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncated_with_ellipsis() {
        assert_eq!(truncated_with_ellipsis("hello", 10), "hello");
        assert_eq!(truncated_with_ellipsis("hello world", 8), "hello...");
        assert_eq!(truncated_with_ellipsis("hello", 3), "...");
        assert_eq!(truncated_with_ellipsis("a", 5), "a");
    }

    #[test]
    fn test_generate_test_name() {
        let mut used_names = HashMap::new();

        let name = generate_test_name(
            &["yes".to_string(), "option1".to_string()],
            0,
            &mut used_names,
        );
        assert_eq!(name, "yes:option1");

        let name = generate_test_name(
            &["my project".to_string(), "v1.0".to_string()],
            1,
            &mut used_names,
        );
        assert_eq!(name, "my-project:v1-0");

        let name1 = generate_test_name(&["test".to_string()], 2, &mut used_names);
        let name2 = generate_test_name(&["test".to_string()], 3, &mut used_names);
        assert_eq!(name1, "test");
        assert_eq!(name2, "test-1");
    }

    #[test]
    fn test_generate_test_name_empty_labels() {
        let mut used_names = HashMap::new();
        let name = generate_test_name(&[], 0, &mut used_names);
        assert_eq!(name, "test0");
    }

    #[test]
    fn test_generate_test_name_truncation() {
        let mut used_names = HashMap::new();
        let long_labels: Vec<String> = (0..10).map(|i| format!("verylonglabel{}", i)).collect();
        let name = generate_test_name(&long_labels, 0, &mut used_names);
        assert!(name.len() <= 80);
    }

    #[test]
    fn test_generate_test_name_truncation_with_collision() {
        let mut used_names = HashMap::new();
        let long_labels: Vec<String> = (0..10).map(|i| format!("verylonglabel{}", i)).collect();

        let name1 = generate_test_name(&long_labels, 0, &mut used_names);
        assert!(name1.len() <= 80);

        let name2 = generate_test_name(&long_labels, 1, &mut used_names);
        assert!(name2.len() <= 80);
        assert!(name2.ends_with("-1"));

        let name3 = generate_test_name(&long_labels, 2, &mut used_names);
        assert!(name3.len() <= 80);
        assert!(name3.ends_with("-2"));
    }

    #[test]
    fn test_get_answer_branches_text() {
        use cyanprompt::domain::models::question::{Question, TextQuestion};

        let q = TextQuestion {
            message: "Enter text".to_string(),
            default: None,
            desc: None,
            initial: None,
            id: "q1".to_string(),
        };

        let branches = get_answer_branches(&Question::Text(q), "seed", "pass", "2024-01-01");
        assert_eq!(branches.len(), 1);
        match &branches[0].0 {
            Answer::String(s) => assert_eq!(s, "seed"),
            _ => panic!("Expected String answer"),
        }
        assert_eq!(branches[0].1, "seed");
    }

    #[test]
    fn test_get_answer_branches_select() {
        use cyanprompt::domain::models::question::{Question, SelectQuestion};

        let q = SelectQuestion {
            message: "Choose".to_string(),
            desc: None,
            options: vec!["opt1".to_string(), "opt2".to_string()],
            id: "q1".to_string(),
        };

        let branches = get_answer_branches(&Question::Select(q), "seed", "pass", "2024-01-01");
        assert_eq!(branches.len(), 2);
        match &branches[0].0 {
            Answer::String(s) => assert_eq!(s, "opt1"),
            _ => panic!("Expected String answer"),
        }
        assert_eq!(branches[0].1, "opt1");
        match &branches[1].0 {
            Answer::String(s) => assert_eq!(s, "opt2"),
            _ => panic!("Expected String answer"),
        }
        assert_eq!(branches[1].1, "opt2");
    }

    #[test]
    fn test_get_answer_branches_confirm() {
        use cyanprompt::domain::models::question::{ConfirmQuestion, Question};

        let q = ConfirmQuestion {
            message: "Confirm?".to_string(),
            desc: None,
            default: None,
            error_message: None,
            id: "q1".to_string(),
        };

        let branches = get_answer_branches(&Question::Confirm(q), "seed", "pass", "2024-01-01");
        assert_eq!(branches.len(), 2);
        match &branches[0].0 {
            Answer::Bool(b) => assert!(*b),
            _ => panic!("Expected Bool answer"),
        }
        assert_eq!(branches[0].1, "yes");
        match &branches[1].0 {
            Answer::Bool(b) => assert!(!*b),
            _ => panic!("Expected Bool answer"),
        }
        assert_eq!(branches[1].1, "no");
    }

    #[test]
    fn test_get_answer_branches_checkbox() {
        use cyanprompt::domain::models::question::{CheckboxQuestion, Question};

        let q = CheckboxQuestion {
            message: "Select".to_string(),
            options: vec!["opt1".to_string(), "opt2".to_string()],
            desc: None,
            id: "q1".to_string(),
        };

        let branches = get_answer_branches(&Question::Checkbox(q), "seed", "pass", "2024-01-01");
        assert_eq!(branches.len(), 4); // none + each option + all

        assert!(matches!(&branches[0].0, Answer::StringArray(v) if v.is_empty()));
        assert!(matches!(&branches[1].0, Answer::StringArray(v) if v.len() == 1 && v[0] == "opt1"));
        assert!(matches!(&branches[2].0, Answer::StringArray(v) if v.len() == 1 && v[0] == "opt2"));
        assert!(matches!(&branches[3].0, Answer::StringArray(v) if v.len() == 2));
    }

    #[test]
    fn test_get_answer_branches_checkbox_empty() {
        use cyanprompt::domain::models::question::{CheckboxQuestion, Question};

        let q = CheckboxQuestion {
            message: "Select".to_string(),
            options: vec![],
            desc: None,
            id: "q1".to_string(),
        };

        let branches = get_answer_branches(&Question::Checkbox(q), "seed", "pass", "2024-01-01");
        assert_eq!(branches.len(), 0);
    }

    #[test]
    fn test_get_answer_branches_checkbox_single_option() {
        use cyanprompt::domain::models::question::{CheckboxQuestion, Question};

        let q = CheckboxQuestion {
            message: "Select".to_string(),
            options: vec!["only".to_string()],
            desc: None,
            id: "q1".to_string(),
        };

        let branches = get_answer_branches(&Question::Checkbox(q), "seed", "pass", "2024-01-01");
        assert_eq!(branches.len(), 2);
        assert!(matches!(&branches[0].0, Answer::StringArray(v) if v.is_empty()));
        assert!(matches!(&branches[1].0, Answer::StringArray(v) if v.len() == 1 && v[0] == "only"));
    }
}
