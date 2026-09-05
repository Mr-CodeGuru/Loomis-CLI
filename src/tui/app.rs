use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use crate::core::LoomisCore;
use crate::db::SearchResult;
use crate::llm::CodeIntent;
use super::markdown::parse_markdown_to_lines;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub enum RagEvent {
    Intent(CodeIntent),
    Chunks(Vec<SearchResult>),
    Token(String),
    Done(String),
    Error(String),
}

pub struct ConversationTurn {
    pub user_query: String,
    pub assistant_response: String,
    pub chunks: Vec<SearchResult>,
    pub intent: Option<CodeIntent>,
    pub is_complete: bool,
}

pub struct TuiApp {
    pub core: LoomisCore,
    pub turns: Vec<ConversationTurn>,
    pub textarea: TextArea<'static>,
    pub prompt_history: Vec<String>,
    pub history_cursor: Option<usize>,
    pub status_text: String,
    pub is_generating: bool,
    pub spinner_idx: usize,
    pub scroll_offset: u16,
    pub auto_scroll: bool,
}

impl TuiApp {
    pub fn new(core: LoomisCore) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text("Ask a question or enter code request... (Enter to send, Esc to exit)");
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Prompt "),
        );

        Self {
            core,
            turns: Vec::new(),
            textarea,
            prompt_history: Vec::new(),
            history_cursor: None,
            status_text: "Ready".to_string(),
            is_generating: false,
            spinner_idx: 0,
            scroll_offset: 0,
            auto_scroll: true,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Setup panic hook to always restore terminal
        let default_panic = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), LeaveAlternateScreen);
            default_panic(panic_info);
        }));

        enable_raw_mode()?;
        let mut stdout_handle = stdout();
        execute!(stdout_handle, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout_handle);
        let mut terminal = Terminal::new(backend)?;

        let (tx, mut rx) = mpsc::unbounded_channel::<RagEvent>();
        let tick_rate = Duration::from_millis(50);

        loop {
            // Check RAG background worker messages
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    RagEvent::Intent(intent) => {
                        if let Some(turn) = self.turns.last_mut() {
                            turn.intent = Some(intent);
                        }
                        self.status_text = match intent {
                            CodeIntent::Chat => "Generating direct chat response...".to_string(),
                            CodeIntent::Code => "Code intent detected. Searching repository...".to_string(),
                        };
                    }
                    RagEvent::Chunks(chunks) => {
                        let count = chunks.len();
                        if let Some(turn) = self.turns.last_mut() {
                            turn.chunks = chunks;
                        }
                        self.status_text = format!("Found {count} context snippets. Streaming response...");
                    }
                    RagEvent::Token(token) => {
                        if let Some(turn) = self.turns.last_mut() {
                            turn.assistant_response.push_str(&token);
                        }
                        if self.auto_scroll {
                            self.scroll_to_bottom();
                        }
                    }
                    RagEvent::Done(full_res) => {
                        if let Some(turn) = self.turns.last_mut() {
                            turn.is_complete = true;
                            // Update core in-memory history
                            self.core.record_turn(&turn.user_query, &full_res);
                        }
                        self.is_generating = false;
                        self.status_text = "Ready".to_string();
                        if self.auto_scroll {
                            self.scroll_to_bottom();
                        }
                    }
                    RagEvent::Error(err) => {
                        if let Some(turn) = self.turns.last_mut() {
                            turn.assistant_response.push_str(&format!("\n\n[ERROR: {err}]"));
                            turn.is_complete = true;
                        }
                        self.is_generating = false;
                        self.status_text = format!("Error: {err}");
                    }
                }
            }

            // Draw frame
            terminal.draw(|f| self.render_ui(f))?;

            // Process crossterm input events
            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key, &tx).await? {
                        break;
                    }
                }
            }

            // Spinner animation step while generating
            if self.is_generating {
                self.spinner_idx = (self.spinner_idx + 1) % SPINNER_FRAMES.len();
            }
        }

        // Clean exit: restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        self.core.sidecar.shutdown().await;

        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        tx: &mpsc::UnboundedSender<RagEvent>,
    ) -> Result<bool> {
        // Ctrl+C or Esc (when input empty) quits
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }
        if key.code == KeyCode::Esc && self.textarea.is_empty() {
            return Ok(true);
        }

        // Scrolling controls
        match key.code {
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_add(5);
                self.auto_scroll = false;
                return Ok(false);
            }
            KeyCode::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_sub(5);
                if self.scroll_offset == 0 {
                    self.auto_scroll = true;
                }
                return Ok(false);
            }
            _ => {}
        }

        // History recall on Up/Down when on first line
        if key.code == KeyCode::Up && self.textarea.cursor().0 == 0 && !self.prompt_history.is_empty() {
            let next_idx = match self.history_cursor {
                None => self.prompt_history.len().saturating_sub(1),
                Some(i) => i.saturating_sub(1),
            };
            self.history_cursor = Some(next_idx);
            let prev_text = &self.prompt_history[next_idx];
            self.textarea = TextArea::from(vec![prev_text.clone()]);
            self.style_textarea();
            return Ok(false);
        }

        if key.code == KeyCode::Down && self.history_cursor.is_some() {
            let next_idx = self.history_cursor.unwrap() + 1;
            if next_idx < self.prompt_history.len() {
                self.history_cursor = Some(next_idx);
                let text = &self.prompt_history[next_idx];
                self.textarea = TextArea::from(vec![text.clone()]);
            } else {
                self.history_cursor = None;
                self.textarea = TextArea::default();
            }
            self.style_textarea();
            return Ok(false);
        }

        // Submit prompt on Enter (without Shift)
        if key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::SHIFT) {
            let query = self.textarea.lines().join("\n").trim().to_string();
            if query.is_empty() || self.is_generating {
                return Ok(false);
            }

            if query == "/exit" || query == "/quit" {
                return Ok(true);
            }

            if query == "/clear" {
                self.turns.clear();
                self.core.history.clear();
                self.textarea = TextArea::default();
                self.style_textarea();
                self.scroll_offset = 0;
                return Ok(false);
            }

            // Save to history
            self.prompt_history.push(query.clone());
            self.history_cursor = None;

            // Push new conversation turn
            self.turns.push(ConversationTurn {
                user_query: query.clone(),
                assistant_response: String::new(),
                chunks: Vec::new(),
                intent: None,
                is_complete: false,
            });

            self.textarea = TextArea::default();
            self.style_textarea();
            self.is_generating = true;
            self.auto_scroll = true;
            self.status_text = "Classifying query intent...".to_string();

            // Spawn async RAG pipeline
            let tx_clone = tx.clone();
            let prep = match self.core.prepare_query(&query).await {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx_clone.send(RagEvent::Error(e.to_string()));
                    return Ok(false);
                }
            };

            let _ = tx_clone.send(RagEvent::Intent(prep.intent));
            if !prep.chunks.is_empty() {
                let _ = tx_clone.send(RagEvent::Chunks(prep.chunks.clone()));
            }

            let llm = self.core.llm.clone();
            tokio::spawn(async move {
                let stream_res = llm
                    .stream_chat(&prep.messages, |token| {
                        let _ = tx_clone.send(RagEvent::Token(token.to_string()));
                        Ok(())
                    })
                    .await;

                match stream_res {
                    Ok(full_res) => {
                        let _ = tx_clone.send(RagEvent::Done(full_res));
                    }
                    Err(e) => {
                        let _ = tx_clone.send(RagEvent::Error(e.to_string()));
                    }
                }
            });

            return Ok(false);
        }

        // Forward other keystrokes to tui-textarea
        self.textarea.input(key);
        Ok(false)
    }

    fn style_textarea(&mut self) {
        self.textarea.set_cursor_line_style(Style::default());
        self.textarea.set_placeholder_text("Ask a question or enter code request... (Enter to send, Esc to exit)");
        self.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Prompt "),
        );
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    fn render_ui(&mut self, f: &mut Frame) {
        let size = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // Conversation pane
                Constraint::Length(1), // Status bar
                Constraint::Length(5), // Input box
            ])
            .split(size);

        self.render_conversation(f, chunks[0]);
        self.render_status_bar(f, chunks[1]);
        f.render_widget(&self.textarea, chunks[2]);
    }

    fn render_conversation(&self, f: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        if self.turns.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("⚡ LoomisCLI TUI", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" — Local-first RAG Code Assistant"),
            ]));
            lines.push(Line::from(Span::styled(
                "Type your query below and press Enter. Powered by LanceDB + Llama-3.2-1B.",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        }

        for turn in &self.turns {
            // User query turn
            lines.push(Line::from(vec![
                Span::styled("❯ You", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(&turn.user_query, Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(""));

            // Retrieved sources indicator
            if !turn.chunks.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  ┌ Sources (", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} snippets", turn.chunks.len()), Style::default().fg(Color::LightCyan)),
                    Span::styled(") ──────────────────────────", Style::default().fg(Color::DarkGray)),
                ]));
                for (i, c) in turn.chunks.iter().enumerate() {
                    let name = if c.extracted_name.is_empty() { "block" } else { &c.extracted_name };
                    lines.push(Line::from(vec![
                        Span::styled(format!("  │  [{}] ", i + 1), Style::default().fg(Color::DarkGray)),
                        Span::styled(&c.path, Style::default().fg(Color::LightYellow)),
                        Span::styled(format!(" ({name}) "), Style::default().fg(Color::White)),
                        Span::styled(format!("[dist: {:.1}]", c.distance), Style::default().fg(Color::DarkGray)),
                    ]));
                }
                lines.push(Line::from(Span::styled("  └────────────────────────────────────────────", Style::default().fg(Color::DarkGray))));
                lines.push(Line::from(""));
            }

            // Assistant response
            lines.push(Line::from(vec![
                Span::styled("◆ Loomis", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ]));

            let response_lines = parse_markdown_to_lines(&turn.assistant_response);
            for l in response_lines {
                let mut spans = vec![Span::raw("  ")];
                spans.extend(l.spans);
                lines.push(Line::from(spans));
            }

            if !turn.is_complete && self.is_generating {
                let spinner = SPINNER_FRAMES[self.spinner_idx];
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{spinner} Generating..."), Style::default().fg(Color::Yellow)),
                ]));
            }

            lines.push(Line::from(""));
        }

        // Calculate visible viewport scrolling
        let total_lines = lines.len() as u16;
        let visible_height = area.height.saturating_sub(2); // padding for borders
        let scroll = if total_lines > visible_height {
            let max_scroll = total_lines.saturating_sub(visible_height);
            max_scroll.saturating_sub(self.scroll_offset)
        } else {
            0
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" Conversation ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));

        f.render_widget(paragraph, area);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let spinner = if self.is_generating {
            format!("{} ", SPINNER_FRAMES[self.spinner_idx])
        } else {
            String::new()
        };

        let status_line = Line::from(vec![
            Span::styled(" Model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.core.config.model, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" │ Endpoint: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.core.config.endpoint_url, Style::default().fg(Color::White)),
            Span::styled(" │ State: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{spinner}{}", self.status_text), Style::default().fg(Color::Yellow)),
            Span::styled(" │ Turns: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", self.turns.len()), Style::default().fg(Color::Cyan)),
        ]);

        let status_bar = Paragraph::new(status_line).style(Style::default().bg(Color::Rgb(25, 28, 35)));
        f.render_widget(status_bar, area);
    }
}
