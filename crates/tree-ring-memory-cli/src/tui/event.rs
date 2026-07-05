#![allow(dead_code)]

use ratatui::crossterm::event::KeyEvent;

use super::stream::LiveEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleEvent {
    Tick,
    Key(KeyEvent),
    StoreRefresh,
    Stream(LiveEvent),
}
