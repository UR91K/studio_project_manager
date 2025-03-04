use iced::{Application, Command, Element, Length, Theme};
use iced::widget::{
    Button, Column, Container, Row, Scrollable, Text, TextInput, Space,
};
use log::{debug, error, info};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::live_set::LiveSet;
use crate::process_projects;
use crate::ui::theme_loader;
use crate::ui::style::{AbletonDividerStyle, AbletonListRowStyle, AbletonPanelStyle};
use crate::ui::style::custom_scrollbar_style;

use super::message::Message;
use super::state::{AppState, UiState, StatusInfo};

pub struct StudioProjectManager {
    // Database connection
    db: Option<Arc<Mutex<LiveSetDatabase>>>,
    db_path: PathBuf,
    
    // Application state
    state: AppState,
    
    // UI state
    ui_state: UiState,
}

impl Application for StudioProjectManager {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let config = match CONFIG.as_ref() {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to load config: {}", e);
                return (
                    Self {
                        db: None,
                        db_path: PathBuf::new(),
                        state: AppState::Error(format!("Failed to load config: {}", e)),
                        ui_state: UiState::default(),
                    },
                    Command::none(),
                );
            }
        };
        
        let db_path = PathBuf::from(&config.database_path);
        
        (
            Self {
                db: None,
                db_path,
                state: AppState::Loading,
                ui_state: UiState::default(),
            },
            Command::perform(async {}, |_| Message::Initialize)
        )
    }
    
    fn title(&self) -> String {
        String::from("Studio Project Manager")
    }
    
    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Initialize => {
                debug!("Initializing application");
                self.ui_state.status.message = "Initializing...".to_string();
                self.ui_state.status.progress = None;
                
                let db_path = self.db_path.clone();
                
                Command::perform(
                    async move {
                        LiveSetDatabase::new(db_path)
                    },
                    |result| match result {
                        Ok(_) => Message::DatabaseLoaded(Ok(())),
                        Err(e) => Message::DatabaseLoaded(Err(e.to_string())),
                    }
                )
            },
            Message::DatabaseLoaded(result) => {
                match result {
                    Ok(()) => {
                        info!("Database loaded successfully");
                        self.ui_state.status.message = "Loading projects...".to_string();
                        
                        // Create the database connection
                        match LiveSetDatabase::new(self.db_path.clone()) {
                            Ok(db) => {
                                self.db = Some(Arc::new(Mutex::new(db)));
                                
                                if let Some(db) = &self.db {
                                    let db_clone = Arc::clone(db);
                                    
                                    Command::perform(
                                        async move {
                                            let db_guard = db_clone.lock().unwrap();
                                            match db_guard.get_all_projects() {
                                                Ok(projects) => Ok(projects),
                                                Err(e) => Err(e.to_string()),
                                            }
                                        },
                                        Message::ProjectsLoaded
                                    )
                                } else {
                                    Command::none()
                                }
                            },
                            Err(e) => {
                                error!("Failed to load database: {}", e);
                                self.ui_state.status.message = format!("Error: {}", e);
                                self.state = AppState::Error(format!("Failed to load database: {}", e));
                                Command::none()
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to load database: {}", e);
                        self.ui_state.status.message = format!("Error: {}", e);
                        self.state = AppState::Error(format!("Failed to load database: {}", e));
                        Command::none()
                    }
                }
            },
            Message::ProjectsLoaded(result) => {
                match result {
                    Ok(projects) => {
                        info!("Loaded {} projects", projects.len());
                        self.ui_state.status.message = format!("Ready - {} projects loaded", projects.len());
                        self.ui_state.status.progress = None;
                        
                        self.state = AppState::Ready {
                            projects,
                            search_results: Vec::new(),
                            selected_project_id: None,
                        };
                        Command::none()
                    },
                    Err(e) => {
                        error!("Failed to load projects: {}", e);
                        self.ui_state.status.message = format!("Error: {}", e);
                        self.state = AppState::Error(format!("Failed to load projects: {}", e));
                        Command::none()
                    }
                }
            },
            Message::ViewAllProjects => {
                if let AppState::Ready { projects, selected_project_id, .. } = &self.state {
                    self.ui_state.status.message = "Showing all projects".to_string();
                    
                    self.state = AppState::Ready {
                        projects: projects.clone(),
                        search_results: Vec::new(),
                        selected_project_id: *selected_project_id,
                    };
                }
                Command::none()
            },
            Message::ProjectSelected(project_id) => {
                if let AppState::Ready { projects, search_results, .. } = &self.state {
                    if let Some(id) = &project_id {
                        self.ui_state.status.message = format!("Selected project: {}", id);
                    } else {
                        self.ui_state.status.message = "No project selected".to_string();
                    }
                    
                    self.state = AppState::Ready {
                        projects: projects.clone(),
                        search_results: search_results.clone(),
                        selected_project_id: project_id,
                    };
                }
                Command::none()
            },
            Message::SearchQueryChanged(query) => {
                self.ui_state.search_query = query.clone();
                
                if query.is_empty() {
                    if let AppState::Ready { projects, selected_project_id, .. } = &self.state {
                        self.ui_state.status.message = "Ready".to_string();
                        
                        self.state = AppState::Ready {
                            projects: projects.clone(),
                            search_results: Vec::new(),
                            selected_project_id: *selected_project_id,
                        };
                    }
                    return Command::none();
                }
                
                self.ui_state.status.message = format!("Searching for '{}'...", query);
                
                if let Some(db) = &self.db {
                    let db_clone = Arc::clone(db);
                    let query_clone = query.clone();
                    
                    Command::perform(
                        async move {
                            let mut db_guard = db_clone.lock().unwrap();
                            match db_guard.search(&query_clone) {
                                Ok(results) => Ok(results),
                                Err(e) => Err(e.to_string()),
                            }
                        },
                        Message::SearchPerformed
                    )
                } else {
                    Command::none()
                }
            },
            Message::SearchPerformed(result) => {
                match result {
                    Ok(search_results) => {
                        self.ui_state.status.message = format!("Found {} results", search_results.len());
                        
                        if let AppState::Ready { projects, selected_project_id, .. } = &self.state {
                            self.state = AppState::Ready {
                                projects: projects.clone(),
                                search_results,
                                selected_project_id: *selected_project_id,
                            };
                        }
                    },
                    Err(e) => {
                        error!("Search failed: {}", e);
                        self.ui_state.status.message = format!("Search error: {}", e);
                        // Keep the current state but show an error message
                    }
                }
                Command::none()
            },
            Message::ScanFoldersClicked => {
                info!("Scanning folders");
                self.ui_state.status.message = "Scanning folders...".to_string();
                self.ui_state.status.progress = Some(0.0);
                
                // Create a channel for progress updates
                let (sender, _receiver) = iced::futures::channel::mpsc::unbounded();
                
                // Set the progress callback
                crate::set_progress_callback(move |progress| {
                    let _ = sender.unbounded_send(progress);
                });
                
                // Create a command that will periodically update progress
                let progress_command = Command::perform(
                    async {
                        // Use std::thread::sleep to simulate initial delay
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        
                        // Return a default progress to start
                        0.1
                    },
                    |progress| Message::ScanProgress(progress)
                );
                
                // Start the actual scan process
                let scan_command = Command::perform(
                    async {
                        let result = process_projects();
                        // Clear the callback when done
                        crate::clear_progress_callback();
                        result
                    },
                    |result| match result {
                        Ok(()) => Message::ScanCompleted(Ok(())),
                        Err(e) => Message::ScanCompleted(Err(e.to_string())),
                    }
                );
                
                Command::batch(vec![progress_command, scan_command])
            },
            Message::ScanProgress(progress) => {
                self.ui_state.status.progress = Some(progress);
                self.ui_state.status.message = format!("Scanning folders... {}%", (progress * 100.0) as i32);
                
                // If progress is less than 100%, schedule another update in case we don't get callbacks
                if progress < 0.99 {
                    Command::perform(
                        async move {
                            // Wait a bit before checking again
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            // Return the current progress + a small increment as a fallback
                            // The real progress from the callback will override this if available
                            (progress + 0.01).min(0.99)
                        },
                        Message::ScanProgress
                    )
                } else {
                    Command::none()
                }
            },
            Message::ScanCompleted(result) => {
                match result {
                    Ok(_) => {
                        info!("Scan completed successfully");
                        self.ui_state.status.message = "Scan completed. Reloading projects...".to_string();
                        self.ui_state.status.progress = None;
                        
                        // Reload projects after scanning
                        if let Some(db) = &self.db {
                            let db_clone = Arc::clone(db);
                            
                            Command::perform(
                                async move {
                                    let db_guard = db_clone.lock().unwrap();
                                    match db_guard.get_all_projects() {
                                        Ok(projects) => Ok(projects),
                                        Err(e) => Err(e.to_string()),
                                    }
                                },
                                Message::ProjectsLoaded
                            )
                        } else {
                            Command::none()
                        }
                    },
                    Err(e) => {
                        error!("Scan failed: {}", e);
                        self.ui_state.status.message = format!("Scan failed: {}", e);
                        self.ui_state.status.progress = None;
                        self.state = AppState::Error(format!("Scan failed: {}", e));
                        Command::none()
                    }
                }
            },
            Message::UpdateStatus(message) => {
                self.ui_state.status.message = message;
                self.ui_state.status.progress = None;
                Command::none()
            },
            Message::UpdateStatusWithProgress(message, progress) => {
                self.ui_state.status.message = message.clone();
                self.ui_state.status.progress = Some(progress);
                
                // If we're still scanning and progress is less than 0.9, continue updating
                if message.contains("Scanning") && progress < 0.9 {
                    let next_progress = (progress + 0.1).min(0.9);
                    
                    Command::perform(
                        async {
                            // Use std::thread::sleep instead of tokio
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        },
                        move |_| Message::UpdateStatusWithProgress("Scanning folders...".to_string(), next_progress)
                    )
                } else {
                    Command::none()
                }
            },
            Message::ClearStatus => {
                self.ui_state.status.message = "Ready".to_string();
                self.ui_state.status.progress = None;
                Command::none()
            },
            Message::LoadTheme(path) => {
                if let Some(path) = path {
                    // If a path is provided, load the theme directly
                    Command::perform(
                        async move {
                            match theme_loader::load_theme(Some(&path)) {
                                Ok(_) => Ok(()),
                                Err(e) => Err(e),
                            }
                        },
                        Message::ThemeLoaded
                    )
                } else {
                    // If no path is provided, we need to show a file dialog
                    // This would typically use a file dialog library like rfd
                    // For now, we'll just load the default theme
                    self.ui_state.status = StatusInfo {
                        message: "Theme selection not implemented yet. Using default theme.".to_string(),
                        progress: None,
                    };
                    Command::perform(
                        async {
                            match theme_loader::load_theme(None) {
                                Ok(_) => Ok(()),
                                Err(e) => Err(e),
                            }
                        },
                        Message::ThemeLoaded
                    )
                }
            },
            Message::ThemeLoaded(result) => {
                match result {
                    Ok(_) => {
                        self.ui_state.status = StatusInfo {
                            message: "Theme loaded successfully".to_string(),
                            progress: None,
                        };
                    },
                    Err(e) => {
                        error!("Failed to load theme: {:?}", e);
                        self.ui_state.status = StatusInfo {
                            message: format!("Failed to load theme: {}", e),
                            progress: None,
                        };
                    }
                }
                Command::none()
            },
        }
    }
    
    fn view(&self) -> Element<Message> {
        match &self.state {
            AppState::Loading => {
                // Simple loading screen
                Container::new(
                    Text::new("Loading...")
                        .size(24)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .style(iced::theme::Container::Custom(Box::new(super::style::AbletonBackgroundStyle)))
                .into()
            },
            AppState::Ready { projects, search_results, selected_project_id } => {
                // Main layout with three panels
                let display_projects = if !search_results.is_empty() {
                    search_results
                } else {
                    projects
                };
                
                // Left panel (navigation)
                let left_panel = self.view_left_panel();
                
                // Center panel (project list)
                let center_panel = self.view_project_list(display_projects);
                
                // Right panel (project details)
                let right_panel = self.view_project_details(selected_project_id, projects);
                
                // Main content
                let content = Row::new()
                    .push(left_panel)
                    .push(center_panel)
                    .push(right_panel)
                    .spacing(10)
                    .height(Length::Fill);
                
                // Top bar with search
                let search_bar = self.view_search_bar();
                
                // Status bar
                let status_bar = self.view_status_bar();
                
                // Combine everything
                let content = Column::new()
                    .push(search_bar)
                    .push(content)
                    .push(status_bar)
                    .spacing(10)
                    .padding(10)
                    .height(Length::Fill);
                
                Container::new(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(iced::theme::Container::Custom(Box::new(super::style::AbletonBackgroundStyle)))
                    .into()
            },
            AppState::Error(error) => {
                // Error screen
                Container::new(
                    Column::new()
                        .push(Text::new("Error").size(24))
                        .push(Text::new(error))
                        .spacing(10)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .style(iced::theme::Container::Custom(Box::new(super::style::AbletonBackgroundStyle)))
                .into()
            }
        }
    }
}

impl StudioProjectManager {
    // Add a static run method that properly calls Application::run
    pub fn run(settings: iced::Settings<()>) -> iced::Result {
        <Self as iced::Application>::run(settings)
    }

    // Helper method to create the left panel
    fn view_left_panel(&self) -> Element<Message> {
        let title = Container::new(
            Text::new("Navigation").size(20)
        )
        .width(Length::Fill)
        .padding(10)
        .style(iced::theme::Container::Custom(Box::new(super::style::AbletonHeaderStyle)));
        
        let content = Column::new()
            .push(Button::new(Text::new("All Projects")).on_press(Message::ViewAllProjects))
            .push(
                Container::new(
                    Text::new("Collections").size(20)
                )
                .width(Length::Fill)
                .padding(10)
                .style(iced::theme::Container::Custom(Box::new(super::style::AbletonHeaderStyle)))
            )
            .push(Button::new(Text::new("Scan Folders")).on_press(Message::ScanFoldersClicked))
            .push(
                Container::new(
                    Text::new("Appearance").size(20)
                )
                .width(Length::Fill)
                .padding(10)
                .style(iced::theme::Container::Custom(Box::new(super::style::AbletonHeaderStyle)))
            )
            .push(
                Button::new(Text::new("Load Theme"))
                    .on_press(Message::LoadTheme(None))
                    .style(iced::theme::Button::Secondary)
            )
            .spacing(10)
            .padding(10);
        
        let column = Column::new()
            .push(title)
            .push(content)
            .spacing(1);
        
        Container::new(column)
            .width(Length::Fixed(200.0))
            .height(Length::Fill)
            .padding(1)
            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonPanelStyle)))
            .into()
    }
    
    // Helper method to create the project list
    fn view_project_list<'a>(&'a self, projects: &'a [LiveSet]) -> Element<'a, Message> {
        let header = Container::new(
            Row::new()
                .push(
                    Text::new("Name")
                        .width(Length::FillPortion(2))
                        .style(super::style::get_color("ControlForeground")),
                )
                .push(
                    Text::new("Path")
                        .width(Length::FillPortion(3))
                        .style(super::style::get_color("ControlForeground")),
                )
                .push(
                    Text::new("Modified")
                        .width(Length::FillPortion(2))
                        .style(super::style::get_color("ControlForeground")),
                ),
        )
        .style(super::style::AbletonHeaderStyle)
        .width(Length::Fill)
        .padding(10);

        let content: Element<'a, _> = if !projects.is_empty() {
            projects
                .iter()
                .enumerate()
                .fold(Column::new().spacing(0), |column, (i, project)| {
                    let row = Row::new()
                        .push(
                            Text::new(&project.name)
                                .width(Length::FillPortion(2))
                                .style(super::style::get_color("ControlForeground")),
                        )
                        .push(
                            Text::new(project.file_path.to_string_lossy().to_string())
                                .width(Length::FillPortion(3))
                                .style(super::style::get_color("ControlForeground")),
                        )
                        .push(
                            Text::new(project.modified_time.format("%Y-%m-%d %H:%M").to_string())
                                .width(Length::FillPortion(2))
                                .style(super::style::get_color("ControlForeground")),
                        )
                        .padding(10);

                    let button = Button::new(row)
                        .width(Length::Fill)
                        .style(if i % 2 == 0 {
                            iced::theme::Button::Custom(Box::new(super::style::AbletonRowStyle1))
                        } else {
                            iced::theme::Button::Custom(Box::new(super::style::AbletonRowStyle2))
                        })
                        .on_press(Message::ProjectSelected(Some(project.id)));

                    column.push(button)
                })
                .into()
        } else {
            Container::new(
                Text::new("No projects found")
                    .style(super::style::get_color("ControlForeground"))
                    .horizontal_alignment(iced::alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .center_x()
            .padding(20)
            .into()
        };

        Container::new(
            Column::new()
                .push(header)
                .push(Scrollable::new(content).height(Length::Fill)),
        )
        .style(super::style::AbletonListRowStyle)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    
    // Helper method to create the project details panel
    fn view_project_details<'a>(&self, selected_project_id: &Option<uuid::Uuid>, projects: &'a [LiveSet]) -> Element<'a, Message> {
        let selected_project = selected_project_id
            .and_then(|id| projects.iter().find(|p| p.id == id));
        
        let content = if let Some(project) = selected_project {
            // Extract file name from path for comparison with project name
            let file_name = project.file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            
            // Check if name is custom (different from file name)
            let is_custom_name = project.name != file_name;
            
            // Format duration if available
            let duration_text = if let Some(duration) = project.estimated_duration {
                let minutes = duration.num_minutes();
                let seconds = duration.num_seconds() % 60;
                format!("{:02}:{:02}", minutes, seconds)
            } else {
                "--:--".to_string()
            };
            
            // Format key signature properly
            let key_text = match &project.key_signature {
                Some(key) => {
                    if key.tonic == crate::models::Tonic::Empty {
                        "".to_string()
                    } else if key.scale == crate::models::Scale::Empty {
                        format!("{}", key.tonic)
                    } else {
                        format!("{} {}", key.tonic, key.scale)
                    }
                },
                None => "".to_string()
            };
            
            // Format full Ableton version
            let ableton_version_text = format!(
                "Live {}.{}.{}{}", 
                project.ableton_version.major, 
                project.ableton_version.minor,
                project.ableton_version.patch,
                if project.ableton_version.beta { " beta" } else { "" }
            );
            
            // Project details
            Column::new()
                // Project title card
                .push(
                    Container::new(
                        Column::new()
                            .push(Text::new(&project.name).size(24))
                            .push(
                                if is_custom_name {
                                    // Show file path in low contrast if name is custom
                                    Text::new(project.file_path.to_string_lossy())
                                        .size(12)
                                        .style(iced::Color::from(super::style::get_color("TextDisabled")))
                                } else {
                                    // Empty text if name is not custom
                                    Text::new("")
                                }
                            )
                            .spacing(5)
                    )
                    .width(Length::Fill)
                    .padding(10)
                    .style(iced::theme::Container::Custom(Box::new(super::style::AbletonHeaderStyle)))
                )
                
                // Metadata section
                .push(
                    Column::new()
                        // Created timestamp
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Created:").width(Length::Fixed(120.0)))
                                    .push(Text::new(project.created_time.format("%Y-%m-%d %H:%M").to_string()))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle1)))
                        )
                        // Modified timestamp
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Modified:").width(Length::Fixed(120.0)))
                                    .push(Text::new(project.modified_time.format("%Y-%m-%d %H:%M").to_string()))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle2)))
                        )
                        // Tempo
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Tempo:").width(Length::Fixed(120.0)))
                                    .push(Text::new(format!("{:.1} bpm", project.tempo)))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle1)))
                        )
                        // Time Signature
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Time Signature:").width(Length::Fixed(120.0)))
                                    .push(Text::new(format!("{}/{}", 
                                        project.time_signature.numerator, 
                                        project.time_signature.denominator
                                    )))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle2)))
                        )
                        // Key
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Key:").width(Length::Fixed(120.0)))
                                    .push(Text::new(key_text))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle1)))
                        )
                        // Duration
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Duration:").width(Length::Fixed(120.0)))
                                    .push(Text::new(duration_text))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle2)))
                        )
                        // Ableton Version
                        .push(
                            Container::new(
                                Row::new()
                                    .push(Text::new("Ableton Version:").width(Length::Fixed(120.0)))
                                    .push(Text::new(ableton_version_text))
                                    .padding(10)
                            )
                            .width(Length::Fill)
                            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle1)))
                        )
                )
                
                // Plugins section
                .push(
                    Container::new(
                        Column::new()
                            .push(Text::new("Plugins:"))
                            .push({
                                let mut plugins_column = Column::new().spacing(5);
                                for plugin in &project.plugins {
                                    let status_symbol = if plugin.installed {
                                        "O"
                                    } else {
                                        "X"
                                    };
                                    
                                    let status_color = if plugin.installed {
                                        iced::color!(0x00d861) // Green for installed
                                    } else {
                                        iced::color!(0xff5559) // Red for not installed
                                    };
                                    
                                    plugins_column = plugins_column.push(
                                        Row::new()
                                            .push(
                                                Text::new(status_symbol)
                                                    .width(Length::Fixed(20.0))
                                                    .style(status_color)
                                            )
                                            .push(
                                                Text::new(&plugin.name)
                                            )
                                    );
                                }
                                Scrollable::new(plugins_column)
                                    .height(Length::Fixed(150.0))
                                    .style(super::style::custom_scrollbar_style())
                            })
                            .spacing(10)
                            .padding(10)
                    )
                    .width(Length::Fill)
                    .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle2)))
                )
                
                // Samples section
                .push(
                    Container::new(
                        Column::new()
                            .push(Text::new("Samples:"))
                            .push({
                                let mut samples_column = Column::new().spacing(5);
                                for sample in &project.samples {
                                    let status_symbol = if sample.is_present {
                                        "O"
                                    } else {
                                        "X"
                                    };
                                    
                                    let status_color = if sample.is_present {
                                        iced::color!(0x00d861) // Green for present
                                    } else {
                                        iced::color!(0xff5559) // Red for missing
                                    };
                                    
                                    samples_column = samples_column.push(
                                        Row::new()
                                            .push(
                                                Text::new(status_symbol)
                                                    .width(Length::Fixed(20.0))
                                                    .style(status_color)
                                            )
                                            .push(
                                                Text::new(&sample.name)
                                            )
                                    );
                                }
                                Scrollable::new(samples_column)
                                    .height(Length::Fixed(150.0))
                                    .style(super::style::custom_scrollbar_style())
                            })
                            .spacing(10)
                            .padding(10)
                    )
                    .width(Length::Fill)
                    .style(iced::theme::Container::Custom(Box::new(super::style::AbletonRowStyle1)))
                )
                .spacing(1)
        } else {
            // No project selected
            Column::new()
                .push(Text::new("Select a project to view details"))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_items(iced::Alignment::Center)
        };
        
        Container::new(content)
            .width(Length::Fixed(350.0))
            .height(Length::Fill)
            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonPanelStyle)))
            .into()
    }
    
    // Helper method to create the search bar
    fn view_search_bar(&self) -> Element<Message> {
        let search_input = TextInput::new(
            "Search projects...",
            &self.ui_state.search_query
        )
        .on_input(Message::SearchQueryChanged)
        .padding(10)
        .width(Length::Fill)
        .style(iced::theme::TextInput::Custom(Box::new(super::style::AbletonTextInputStyle)));
        
        Container::new(
            Row::new()
                .push(search_input)
                .spacing(10)
                .padding(10)
                .width(Length::Fill)
        )
        .max_width(600.0)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .style(iced::theme::Container::Custom(Box::new(super::style::AbletonPanelStyle)))
        .into()
    }

    // Helper method to create the status bar
    fn view_status_bar(&self) -> Element<Message> {
        let status_text = Text::new(&self.ui_state.status.message)
            .size(14);
        
        let status_row = if let Some(progress) = self.ui_state.status.progress {
            // If we have progress, show a progress bar
            let progress_bar = iced::widget::ProgressBar::new(0.0..=1.0, progress)
                .width(Length::Fixed(200.0));
                
            Row::new()
                .push(status_text)
                .push(iced::widget::Space::with_width(Length::Fill))
                .push(progress_bar)
                .align_items(iced::Alignment::Center)
        } else {
            // Otherwise just show the status text
            Row::new()
                .push(status_text)
                .align_items(iced::Alignment::Center)
        };
        
        Container::new(status_row)
            .width(Length::Fill)
            .padding(7)
            .style(iced::theme::Container::Custom(Box::new(super::style::AbletonHeaderStyle)))
            .into()
    }
} 