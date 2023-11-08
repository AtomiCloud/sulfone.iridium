pub enum Question {
    Confirm(ConfirmQuestion),
    Date(DateQuestion),
    Checkbox(CheckboxQuestion),
    Password(PasswordQuestion),
    Text(TextQuestion),
    Select(SelectQuestion),
}

pub struct ConfirmQuestion {
    pub message: String,
    pub desc: Option<String>,
    pub default: Option<bool>,
    pub error_message: Option<String>,
}

pub struct DateQuestion {
    pub message: String,
    pub desc: Option<String>,
    pub default: Option<String>,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
}

pub struct CheckboxQuestion {
    pub message: String,
    pub options: Vec<String>,
    pub desc: Option<String>,
}

pub struct PasswordQuestion {
    pub message: String,
    pub desc: Option<String>,
    pub confirmation: Option<bool>,
}

pub struct TextQuestion {
    pub message: String,
    pub default: Option<String>,
    pub desc: Option<String>,
    pub initial: Option<String>,
}

pub struct SelectQuestion {
    pub message: String,
    pub desc: Option<String>,
    pub options: Vec<String>,
}
