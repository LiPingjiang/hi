//! AI mode — input accumulation (network request handled async in app.rs).
use crossterm::event::{KeyCode, KeyEvent};

pub enum AiInputAction {
    None,
    Submit(String),
    Cancel,
    ConfirmGhost,  // Tab pressed while ghost is showing
    ConfirmPlan,   // y pressed while plan is showing
    CancelPlan,    // n pressed while plan is showing
}

pub fn handle_ai_input_key(input: &mut String, key: KeyEvent) -> AiInputAction {
    match key.code {
        KeyCode::Esc => {
            input.clear();
            AiInputAction::Cancel
        }
        KeyCode::Enter => {
            let query = input.trim().to_string();
            input.clear();
            if query.is_empty() {
                AiInputAction::Cancel
            } else {
                AiInputAction::Submit(query)
            }
        }
        KeyCode::Backspace => {
            input.pop();
            AiInputAction::None
        }
        KeyCode::Tab => AiInputAction::ConfirmGhost,
        KeyCode::Char('y') => {
            // Ambiguous: could be typing or confirming plan.
            // The caller (app.rs) checks whether a plan is showing.
            AiInputAction::ConfirmPlan
        }
        KeyCode::Char('n') => AiInputAction::CancelPlan,
        KeyCode::Char(c) => {
            input.push(c);
            AiInputAction::None
        }
        _ => AiInputAction::None,
    }
}
