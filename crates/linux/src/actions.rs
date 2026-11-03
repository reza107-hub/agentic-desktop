//! Perform actions on an AT-SPI element.

use crate::bus::{ACTION, Atspi, EDITABLE_TEXT, TEXT, Target};
use agentic_desktop_core::{Action, ActionReport, AppError, ErrorCode};

/// Preferred names for the "default" press-like action, in order.
const CLICK_NAMES: &[&str] = &[
    "click", "press", "activate", "toggle", "open", "jump", "select",
];

fn failed(message: &str) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

fn unsupported(what: &str) -> AppError {
    AppError::new(
        ErrorCode::ActionNotSupported,
        format!("This element cannot {what}"),
    )
    .with_suggestion("Take a snapshot and pick an element whose available_actions fit")
}

impl Atspi {
    pub fn perform_on(&self, target: &Target, action: &Action) -> Result<ActionReport, AppError> {
        match action {
            Action::Click => self.click(target)?,
            Action::Type(text) => self.type_text(target, text)?,
            Action::SetValue(value) => self.set_value(target, value)?,
        }
        Ok(ActionReport::unverified())
    }

    fn click(&self, target: &Target) -> Result<(), AppError> {
        let names = self.action_names(target);
        if names.is_empty() {
            return Err(unsupported("be clicked"));
        }
        let index = CLICK_NAMES
            .iter()
            .find_map(|wanted| names.iter().position(|name| name == wanted))
            .unwrap_or(0);
        let accepted: bool = self.call(target, ACTION, "DoAction", &(index as i32,))?;
        accepted
            .then_some(())
            .ok_or_else(|| failed("The app refused the action"))
    }

    fn type_text(&self, target: &Target, text: &str) -> Result<(), AppError> {
        let end = self
            .int_property(target, TEXT, "CharacterCount")
            .map_err(|_| unsupported("take typed text"))?;
        let length = text.chars().count() as i32;
        let accepted: bool = self
            .call(target, EDITABLE_TEXT, "InsertText", &(end, text, length))
            .map_err(|_| unsupported("take typed text"))?;
        accepted
            .then_some(())
            .ok_or_else(|| failed("The app refused the text"))
    }

    fn set_value(&self, target: &Target, value: &str) -> Result<(), AppError> {
        let accepted: bool = self
            .call(target, EDITABLE_TEXT, "SetTextContents", &(value,))
            .map_err(|_| unsupported("have its value replaced"))?;
        accepted
            .then_some(())
            .ok_or_else(|| failed("The app refused the new value"))
    }
}
