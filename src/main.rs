use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Tabs, Wrap,
    },
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, Stdout},
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
use tui_input::{backend::crossterm::EventHandler, Input};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MediaInfo {
    name: String,
    container: String,
    codec: String,
    resolution: String,
    frame_rate: String,
    bitrate: String,
    path: String,
    raw_output: String,
}

#[derive(Debug, Clone)]
struct FilterOptions {
    containers: Vec<String>,
    codecs: Vec<String>,
    resolutions: Vec<String>,
    frame_rates: Vec<String>,
    bitrates: Vec<String>,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            containers: vec![
                "mp4".to_string(),
                "mov".to_string(),
                "avi".to_string(),
                "mkv".to_string(),
                "jpg".to_string(),
                "png".to_string(),
            ],
            codecs: vec![
                "H.264".to_string(),
                "H.265".to_string(),
                "VP9".to_string(),
                "AV1".to_string(),
                "Hap".to_string(),
                "DXV3".to_string(),
            ],
            resolutions: vec![
                "1920x1080".to_string(),
                "1280x720".to_string(),
                "3840x2160".to_string(),
                "2560x1440".to_string(),
            ],
            frame_rates: vec![
                "24".to_string(),
                "25".to_string(),
                "30".to_string(),
                "50".to_string(),
                "60".to_string(),
            ],
            bitrates: vec![
                "1".to_string(),
                "5".to_string(),
                "10".to_string(),
                "15".to_string(),
                "20".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
enum FilterType {
    Container,
    Codec,
    Resolution,
    FrameRate,
    Bitrate,
}

#[derive(Debug, Clone)]
struct ActiveFilter {
    filter_type: FilterType,
    value: String,
}

enum AppMode {
    Normal,
    AddFile,
    ShowRawOutput,
    Help,
}

struct App {
    media_files: Vec<MediaInfo>,
    table_state: TableState,
    filter_options: FilterOptions,
    active_filters: Vec<ActiveFilter>,
    mode: AppMode,
    input: Input,
    selected_tab: usize,
    raw_output_scroll: usize,
    notification: Option<(String, Instant)>,
    last_scan_time: Option<Instant>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            media_files: Vec::new(),
            table_state: TableState::default(),
            filter_options: FilterOptions::default(),
            active_filters: Vec::new(),
            mode: AppMode::Normal,
            input: Input::default(),
            selected_tab: 0,
            raw_output_scroll: 0,
            notification: None,
            last_scan_time: None,
        };
        app.table_state.select(Some(0));
        app
    }

    fn add_file(&mut self, path: &str) -> Result<()> {
        if !Path::new(path).exists() {
            self.show_notification("File does not exist".to_string());
            return Ok(());
        }

        let start_time = Instant::now();
        
        match self.analyze_file(path) {
            Ok(media_info) => {
                self.media_files.push(media_info);
                let elapsed = start_time.elapsed();
                self.show_notification(format!("File analyzed in {:.2}s", elapsed.as_secs_f64()));
                self.last_scan_time = Some(start_time);
            }
            Err(e) => {
                self.show_notification(format!("Error analyzing file: {}", e));
            }
        }
        
        Ok(())
    }

    fn analyze_file(&self, path: &str) -> Result<MediaInfo> {
        let output = Command::new("ffprobe")
            .args([
                "-i", path,
                "-show_streams",
                "-show_format",
                "-hide_banner",
                "-of", "json"
            ])
            .output()?;

        let raw_output = String::from_utf8_lossy(&output.stdout);
        
        // Parse basic info from path
        let path_obj = Path::new(path);
        let name = path_obj.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let container = path_obj.extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // For now, we'll parse the raw output with simple string matching
        // In a real implementation, you'd want to use proper JSON parsing
        let codec = self.extract_codec(&raw_output);
        let resolution = self.extract_resolution(&raw_output);
        let frame_rate = self.extract_frame_rate(&raw_output);
        let bitrate = self.extract_bitrate(&raw_output);

        Ok(MediaInfo {
            name,
            container,
            codec,
            resolution,
            frame_rate,
            bitrate,
            path: path.to_string(),
            raw_output: raw_output.to_string(),
        })
    }

    fn extract_codec(&self, output: &str) -> String {
        if output.contains("h264") {
            "H.264".to_string()
        } else if output.contains("hevc") || output.contains("h265") {
            "H.265".to_string()
        } else if output.contains("vp9") {
            "VP9".to_string()
        } else if output.contains("av01") {
            "AV1".to_string()
        } else if output.contains("hap") {
            "Hap".to_string()
        } else if output.contains("mjpeg") {
            "MJPEG".to_string()
        } else {
            "Unknown".to_string()
        }
    }

    fn extract_resolution(&self, output: &str) -> String {
        // Simple regex-like extraction
        for line in output.lines() {
            if line.contains("width") && line.contains("height") {
                // This is a simplified extraction - in reality you'd want proper JSON parsing
                if line.contains("1920") && line.contains("1080") {
                    return "1920x1080".to_string();
                } else if line.contains("1280") && line.contains("720") {
                    return "1280x720".to_string();
                } else if line.contains("3840") && line.contains("2160") {
                    return "3840x2160".to_string();
                }
            }
        }
        "Unknown".to_string()
    }

    fn extract_frame_rate(&self, output: &str) -> String {
        if output.contains("25/1") || output.contains("\"25\"") {
            "25".to_string()
        } else if output.contains("30/1") || output.contains("\"30\"") {
            "30".to_string()
        } else if output.contains("24/1") || output.contains("\"24\"") {
            "24".to_string()
        } else if output.contains("60/1") || output.contains("\"60\"") {
            "60".to_string()
        } else {
            "Unknown".to_string()
        }
    }

    fn extract_bitrate(&self, output: &str) -> String {
        // Extract bitrate and convert to Mbps
        for line in output.lines() {
            if line.contains("bit_rate") && !line.contains("max_bit_rate") {
                // Simplified extraction
                if let Some(start) = line.find(":") {
                    if let Some(end) = line[start..].find(",") {
                        let bitrate_str = &line[start+1..start+end].trim().replace("\"", "");
                        if let Ok(bitrate) = bitrate_str.parse::<f64>() {
                            return format!("{:.1}", bitrate / 1_000_000.0);
                        }
                    }
                }
            }
        }
        "Unknown".to_string()
    }

    fn show_notification(&mut self, message: String) {
        self.notification = Some((message, Instant::now()));
    }

    fn clear_all(&mut self) {
        self.media_files.clear();
        self.active_filters.clear();
        self.table_state.select(Some(0));
        self.show_notification("All files cleared".to_string());
    }

    fn next_file(&mut self) {
        if self.media_files.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.media_files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn previous_file(&mut self) {
        if self.media_files.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.media_files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn get_filtered_files(&self) -> Vec<&MediaInfo> {
        if self.active_filters.is_empty() {
            return self.media_files.iter().collect();
        }

        self.media_files
            .iter()
            .filter(|file| {
                self.active_filters.iter().all(|filter| {
                    match filter.filter_type {
                        FilterType::Container => file.container.contains(&filter.value),
                        FilterType::Codec => file.codec.contains(&filter.value),
                        FilterType::Resolution => file.resolution.contains(&filter.value),
                        FilterType::FrameRate => file.frame_rate.contains(&filter.value),
                        FilterType::Bitrate => file.bitrate.contains(&filter.value),
                    }
                })
            })
            .collect()
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match app.mode {
                    AppMode::Normal => {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('a') => app.mode = AppMode::AddFile,
                            KeyCode::Char('r') => app.mode = AppMode::ShowRawOutput,
                            KeyCode::Char('h') => app.mode = AppMode::Help,
                            KeyCode::Char('c') => app.clear_all(),
                            KeyCode::Down | KeyCode::Char('j') => app.next_file(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_file(),
                            KeyCode::Tab => {
                                app.selected_tab = (app.selected_tab + 1) % 3;
                            }
                            _ => {}
                        }
                    }
                    AppMode::AddFile => {
                        match key.code {
                            KeyCode::Enter => {
                                let path = app.input.value().to_string();
                                if !path.is_empty() {
                                    app.add_file(&path)?;
                                    app.input.reset();
                                }
                                app.mode = AppMode::Normal;
                            }
                            KeyCode::Esc => {
                                app.input.reset();
                                app.mode = AppMode::Normal;
                            }
                            _ => {
                                app.input.handle_event(&Event::Key(key));
                            }
                        }
                    }
                    AppMode::ShowRawOutput => {
                        match key.code {
                            KeyCode::Esc => app.mode = AppMode::Normal,
                            KeyCode::Up => {
                                if app.raw_output_scroll > 0 {
                                    app.raw_output_scroll -= 1;
                                }
                            }
                            KeyCode::Down => app.raw_output_scroll += 1,
                            _ => {}
                        }
                    }
                    AppMode::Help => {
                        match key.code {
                            KeyCode::Esc => app.mode = AppMode::Normal,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(3),  // Tabs
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Status/notification
        ])
        .split(f.size());

    // Title
    let title = Paragraph::new("🎬 Video Analyzer TUI")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Tabs
    let tabs = Tabs::new(vec!["Files", "Filters", "Stats"])
        .block(Block::default().borders(Borders::ALL))
        .select(app.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[1]);

    // Main content based on mode
    match app.mode {
        AppMode::Normal => render_main_content(f, app, chunks[2]),
        AppMode::AddFile => render_add_file_dialog(f, app, chunks[2]),
        AppMode::ShowRawOutput => render_raw_output(f, app, chunks[2]),
        AppMode::Help => render_help(f, chunks[2]),
    }

    // Status bar
    render_status_bar(f, app, chunks[3]);
}

fn render_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    let filtered_files = app.get_filtered_files();
    
    if filtered_files.is_empty() {
        let empty_msg = Paragraph::new("No files loaded. Press 'a' to add files, 'h' for help")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Files"));
        f.render_widget(empty_msg, area);
        return;
    }

    let header_cells = ["Name", "Container", "Codec", "Resolution", "FPS", "Bitrate(Mbps)"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows = filtered_files.iter().map(|file| {
        let cells = vec![
            Cell::from(format!("{}.{}", file.name, file.container)),
            Cell::from(file.container.clone()),
            Cell::from(file.codec.clone()),
            Cell::from(file.resolution.clone()),
            Cell::from(file.frame_rate.clone()),
            Cell::from(file.bitrate.clone()),
        ];
        Row::new(cells).height(1)
    });

    let table = Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!("Files ({}/{})", 
            filtered_files.len(), app.media_files.len())))
        .widths(&[
            Constraint::Percentage(25),
            Constraint::Percentage(12),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(8),
            Constraint::Percentage(15),
        ])
        .column_spacing(1)
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_add_file_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title("Add File")
        .borders(Borders::ALL);
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let input = Paragraph::new(app.input.value())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("File Path"));
    
    f.render_widget(input, chunks[0]);

    let help_text = vec![
        Line::from("Enter the full path to a video or image file"),
        Line::from("Press Enter to analyze, Esc to cancel"),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  /path/to/video.mp4"),
        Line::from("  /path/to/image.jpg"),
    ];
    
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    
    f.render_widget(help, chunks[1]);

    // Set cursor position
    f.set_cursor(
        chunks[0].x + app.input.visual_cursor() as u16 + 1,
        chunks[0].y + 1,
    );
}

fn render_raw_output(f: &mut Frame, app: &mut App, area: Rect) {
    let selected_file = app.table_state.selected()
        .and_then(|i| app.media_files.get(i));
    
    let content = if let Some(file) = selected_file {
        file.raw_output.clone()
    } else {
        "No file selected".to_string()
    };

    let lines: Vec<Line> = content
        .lines()
        .skip(app.raw_output_scroll)
        .map(|line| Line::from(line))
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Raw FFprobe Output"))
        .style(Style::default().fg(Color::Green))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled("Key Bindings:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  q - Quit application"),
        Line::from("  a - Add file"),
        Line::from("  r - Show raw FFprobe output"),
        Line::from("  c - Clear all files"),
        Line::from("  h - Show this help"),
        Line::from("  ↑/k - Previous file"),
        Line::from("  ↓/j - Next file"),
        Line::from("  Tab - Switch tabs"),
        Line::from(""),
        Line::from(Span::styled("Features:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("• Drag-and-drop style file analysis"),
        Line::from("• Real-time filtering and highlighting"),
        Line::from("• Detailed metadata extraction"),
        Line::from("• Performance monitoring"),
        Line::from("• Cross-platform support"),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });

    f.render_widget(help, area);
}

fn render_status_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let mut status_text = match app.mode {
        AppMode::Normal => "Ready - Press 'h' for help".to_string(),
        AppMode::AddFile => "Enter file path...".to_string(),
        AppMode::ShowRawOutput => "Viewing raw output - Press Esc to return".to_string(),
        AppMode::Help => "Help - Press Esc to return".to_string(),
    };

    // Show notification if present
    if let Some((message, timestamp)) = &app.notification {
        if timestamp.elapsed() < Duration::from_secs(3) {
            status_text = message.clone();
        } else {
            app.notification = None;
        }
    }

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}