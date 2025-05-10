use inquire::{Confirm, DateSelect, MultiSelect, Password, Select, Text};

pub enum Prompts<'a> {
    Text(Text<'a>),
    Confirm(Confirm<'a>),
    Checkbox(MultiSelect<'a, String>),
    Select(Select<'a, String>),
    Password(Password<'a>),
    Date(DateSelect<'a>),
}
