use iced::{Application, Command, Element, Length, Theme};
use iced::widget::{Button, Column, Container, Row, Scrollable, Text, TextInput};
use log::{debug, error, info};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use futures::stream::StreamExt;

use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::process_projects;

use super::message::Message;
use super::state::{AppState, UiState};
use super::style;

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
                    .height(Length::Fill);
                
                // Top bar with search
                let search_bar = self.view_search_bar();
                
                // Status bar
                let status_bar = self.view_status_bar();
                
                // Combine everything
                Container::new(
                    Column::new()
                        .push(search_bar)
                        .push(content)
                        .push(status_bar)
                        .spacing(10)
                        .height(Length::Fill)
                )
                .width(Length::Fill)
                .height(Length::Fill)
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
        let column = Column::new()
            .push(Text::new("Navigation").size(20))
            .push(Button::new(Text::new("All Projects")).on_press(Message::ViewAllProjects))
            .push(Text::new("Collections").size(20))
            .push(Button::new(Text::new("Scan Folders")).on_press(Message::ScanFoldersClicked))
            .spacing(10)
            .padding(20);
        
        Container::new(column)
            .width(Length::Fixed(200.0))
            .height(Length::Fill)
            .style(iced::theme::Container::Box)
            .into()
    }
    
    // Helper method to create the project list
    fn view_project_list<'a>(&self, projects: &'a [LiveSet]) -> Element<'a, Message> {
        // Create column headers
        let header_row = Row::new()
            .push(Text::new("Name").width(Length::Fill))
            .push(Text::new("DAW").width(Length::Fixed(80.0)))
            .push(Text::new("Tempo").width(Length::Fixed(80.0)))
            .push(Text::new("Key").width(Length::Fixed(80.0)))
            .push(Text::new("Modified").width(Length::Fixed(120.0)))
            .padding(10);
        
        // Create scrollable list of projects
        let mut project_list = Column::new().spacing(2);
        
        for project in projects {
            let row = Button::new(
                Row::new()
                    .push(Text::new(&project.name).width(Length::Fill))
                    .push(Text::new(format!("{}.{}", 
                        project.ableton_version.major, 
                        project.ableton_version.minor
                    )).width(Length::Fixed(80.0)))
                    .push(Text::new(format!("{:.1}", project.tempo))
                        .width(Length::Fixed(80.0)))
                    .push(Text::new(
                        project.key_signature
                            .as_ref()
                            .map(|k| k.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ).width(Length::Fixed(80.0)))
                    .push(Text::new(
                        project.modified_time.format("%Y-%m-%d").to_string()
                    ).width(Length::Fixed(120.0)))
                    .padding(5)
            )
            .on_press(Message::ProjectSelected(Some(project.id)))
            .width(Length::Fill);
            
            project_list = project_list.push(row);
        }
        
        let scrollable_list = Scrollable::new(project_list)
            .height(Length::Fill)
            .width(Length::Fill);
        
        Container::new(
            Column::new()
                .push(header_row)
                .push(scrollable_list)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(iced::theme::Container::Box)
        .into()
    }
    
    // Helper method to create the project details panel
    fn view_project_details<'a>(&self, selected_project_id: &Option<uuid::Uuid>, projects: &'a [LiveSet]) -> Element<'a, Message> {
        let selected_project = selected_project_id
            .and_then(|id| projects.iter().find(|p| p.id == id));
        
        let content = if let Some(project) = selected_project {
            // Project details
            Column::new()
                .push(Text::new(&project.name).size(24))
                .push(Text::new(project.file_path.to_string_lossy()).size(12))
                .push(
                    Column::new()
                        .push(Text::new("METADATA").size(14))
                        .push(Row::new()
                            .push(Text::new("Created:").width(Length::Fixed(80.0)))
                            .push(Text::new(project.created_time.format("%Y-%m-%d %H:%M").to_string()))
                        )
                        .push(Row::new()
                            .push(Text::new("Modified:").width(Length::Fixed(80.0)))
                            .push(Text::new(project.modified_time.format("%Y-%m-%d %H:%M").to_string()))
                        )
                        .push(Row::new()
                            .push(Text::new("Tempo:").width(Length::Fixed(80.0)))
                            .push(Text::new(format!("{:.1} BPM", project.tempo)))
                        )
                        .push(Row::new()
                            .push(Text::new("Key:").width(Length::Fixed(80.0)))
                            .push(Text::new(
                                project.key_signature
                                    .as_ref()
                                    .map(|k| k.to_string())
                                    .unwrap_or_else(|| "-".to_string())
                            ))
                        )
                        .spacing(5)
                )
                .push(
                    Column::new()
                        .push(Text::new("PLUGINS").size(14))
                        .push({
                            let mut plugins_column = Column::new().spacing(2);
                            for plugin in &project.plugins {
                                plugins_column = plugins_column.push(Text::new(&plugin.name));
                            }
                            Scrollable::new(plugins_column).height(Length::Fixed(150.0))
                        })
                        .spacing(5)
                )
                .spacing(20)
                .padding(20)
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
            .style(iced::theme::Container::Box)
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
        .width(Length::Fill);
        
        let scan_button = Button::new(
            Text::new("Scan")
        )
        .on_press(Message::ScanFoldersClicked);
        
        Container::new(
            Row::new()
                .push(search_input)
                .push(scan_button)
                .spacing(10)
                .padding(10)
        )
        .width(Length::Fill)
        .style(iced::theme::Container::Box)
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
            .padding(10)
            .style(iced::theme::Container::Custom(Box::new(StatusBarStyle)))
            .into()
    }
}

// Custom style for the status bar to make it more visible
struct StatusBarStyle;

impl iced::widget::container::StyleSheet for StatusBarStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            text_color: Some(iced::Color::from_rgb(0.0, 0.0, 0.0)),
            background: Some(iced::Background::Color(iced::Color::from_rgb(0.9, 0.9, 0.95))),
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: iced::Color::from_rgb(0.8, 0.8, 0.85),
        }
    }
} 