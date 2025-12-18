use std::collections::HashMap;

pub type CommandFn = Box<dyn Fn(&[&str]) -> String>;

pub struct Console {
    commands: HashMap<String, CommandFn>,
    history: Vec<String>,
    cvars: HashMap<String, String>,
}

impl Console {
    pub fn new() -> Self {
        let mut console = Self {
            commands: HashMap::new(),
            history: Vec::new(),
            cvars: HashMap::new(),
        };
        
        console.register_default_commands();
        console
    }

    fn register_default_commands(&mut self) {
        self.register_command("help", Box::new(|_| {
            "Available commands: help, echo, set, get".to_string()
        }));

        self.register_command("echo", Box::new(|args| {
            args.join(" ")
        }));
    }

    pub fn register_command(&mut self, name: &str, func: CommandFn) {
        self.commands.insert(name.to_string(), func);
    }

    pub fn execute(&mut self, command: &str) -> String {
        self.history.push(command.to_string());
        
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return String::new();
        }

        let cmd_name = parts[0];
        let args = &parts[1..];

        if let Some(func) = self.commands.get(cmd_name) {
            func(args)
        } else {
            format!("Unknown command: {}", cmd_name)
        }
    }

    pub fn set_cvar(&mut self, name: &str, value: &str) {
        self.cvars.insert(name.to_string(), value.to_string());
    }

    pub fn get_cvar(&self, name: &str) -> Option<&String> {
        self.cvars.get(name)
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}



