use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuType {
    Main,
    MapSelect,
}

pub struct MenuState {
    pub current_menu: MenuType,
    pub main_menu_selected: usize,
    pub map_menu_selected: usize,
    pub available_maps: Vec<String>,
    pub time: f32,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            current_menu: MenuType::Main,
            main_menu_selected: 0,
            map_menu_selected: 0,
            available_maps: Self::list_available_maps(),
            time: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
    }

    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) -> Option<MenuAction> {
        if !pressed {
            return None;
        }

        match self.current_menu {
            MenuType::Main => self.handle_main_menu_input(key),
            MenuType::MapSelect => self.handle_map_select_input(key),
        }
    }

    fn handle_main_menu_input(&mut self, key: KeyCode) -> Option<MenuAction> {
        let menu_items_count = 2;

        match key {
            KeyCode::ArrowDown => {
                self.main_menu_selected = (self.main_menu_selected + 1) % menu_items_count;
                None
            }
            KeyCode::ArrowUp => {
                self.main_menu_selected = if self.main_menu_selected == 0 {
                    menu_items_count - 1
                } else {
                    self.main_menu_selected - 1
                };
                None
            }
            KeyCode::Enter => {
                match self.main_menu_selected {
                    0 => {
                        self.current_menu = MenuType::MapSelect;
                        None
                    }
                    1 => Some(MenuAction::Quit),
                    _ => None,
                }
            }
            KeyCode::Escape => Some(MenuAction::Quit),
            _ => None,
        }
    }

    fn handle_map_select_input(&mut self, key: KeyCode) -> Option<MenuAction> {
        if self.available_maps.is_empty() {
            return None;
        }

        match key {
            KeyCode::ArrowDown => {
                self.map_menu_selected = (self.map_menu_selected + 1) % self.available_maps.len();
                None
            }
            KeyCode::ArrowUp => {
                self.map_menu_selected = if self.map_menu_selected == 0 {
                    self.available_maps.len() - 1
                } else {
                    self.map_menu_selected - 1
                };
                None
            }
            KeyCode::Enter => {
                let map_name = self.available_maps[self.map_menu_selected].clone();
                Some(MenuAction::StartGame { map: map_name })
            }
            KeyCode::Escape => {
                self.current_menu = MenuType::Main;
                None
            }
            _ => None,
        }
    }

    fn list_available_maps() -> Vec<String> {
        let maps_dir = "maps";
        let mut maps = Vec::new();

        if let Ok(dir) = std::fs::read_dir(maps_dir) {
            for entry in dir.flatten() {
                if let Ok(ft) = entry.file_type() {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if ft.is_file() 
                        && file_name.ends_with(".json")
                        && !file_name.ends_with("_navgraph.json")
                        && !file_name.ends_with("_defrag.json")
                    {
                        let map_name = file_name.trim_end_matches(".json").to_string();
                        maps.push(map_name);
                    }
                }
            }
        }

        if maps.is_empty() {
            maps.push("default".to_string());
        }

        maps.sort();
        maps
    }

    pub fn get_main_menu_items(&self) -> &[&str] {
        &["START", "QUIT"]
    }

    pub fn get_selected_map(&self) -> Option<&str> {
        self.available_maps.get(self.map_menu_selected).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub enum MenuAction {
    StartGame { map: String },
    Quit,
}


