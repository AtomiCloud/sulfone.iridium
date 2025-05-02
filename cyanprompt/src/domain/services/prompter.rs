use crate::domain::models::answer::Answer;
use crate::domain::models::prompt::Prompts;

pub fn prompt(p: Prompts) -> Result<Option<Answer>, Box<dyn std::error::Error + Send>> {
    match p {
        Prompts::Text(text) => text
            .prompt_skippable()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
            .map(|a| a.map(Answer::String)),
        Prompts::Confirm(cfm) => cfm
            .prompt_skippable()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
            .map(|a| a.map(Answer::Bool)),
        Prompts::Checkbox(cb) => cb
            .prompt_skippable()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
            .map(|a| a.map(Answer::StringArray)),
        Prompts::Select(s) => s
            .prompt_skippable()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
            .map(|a| a.map(Answer::String)),
        Prompts::Password(pw) => pw
            .prompt_skippable()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
            .map(|a| a.map(Answer::String)),
        Prompts::Date(d) => d
            .prompt_skippable()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
            .map(|a| a.map(|b| Answer::String(b.format("%Y-%m-%d").to_string()))),
    }
}
