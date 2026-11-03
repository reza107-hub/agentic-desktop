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
            .map_err(|cause| editing_error("take typed text", &cause))?;
        let length = text.chars().count() as i32;
        let accepted: bool = self
            .call(target, EDITABLE_TEXT, "InsertText", &(end, text, length))
            .map_err(|cause| editing_error("take typed text", &cause))?;
        accepted
            .then_some(())
            .ok_or_else(|| failed("The app refused the text"))
    }

    fn set_value(&self, target: &Target, value: &str) -> Result<(), AppError> {
        let accepted: bool = self
            .call(target, EDITABLE_TEXT, "SetTextContents", &(value,))
            .map_err(|cause| editing_error("have its value replaced", &cause))?;
        accepted
            .then_some(())
            .ok_or_else(|| failed("The app refused the new value"))
    }
}

/// True when the failure means the element has no editable-text interface at all.
pub fn lacks_editable_text(message: &str) -> bool {
    let lowered = message.to_lowercase();
    [
        "unknowninterface",
        "unknown interface",
        "unknownmethod",
        "unknown method",
        "no such interface",
        "no such method",
        "notimplemented",
    ]
    .iter()
    .any(|phrase| lowered.contains(phrase))
}

/// Keep the real cause. "Not supported" is only right when the element truly lacks
/// the interface; a timeout or a refusal is something else and must say so.
fn editing_error(what: &str, cause: &AppError) -> AppError {
    if lacks_editable_text(&cause.message) {
        return AppError::new(
            ErrorCode::ActionNotSupported,
            format!("This element cannot {what}: it has no editable-text interface"),
        )
        .with_suggestion(
            "Check available_actions for set-value or type. For a field the app draws itself, click it and use type-text with --headed",
        );
    }
    if cause.code == ErrorCode::Timeout {
        return cause.clone();
    }
    AppError::new(
        ErrorCode::ActionFailed,
        format!("The element could not {what}: {}", cause.message),
    )
    .with_suggestion(
        "Take a new snapshot; if it keeps failing, click the field and use type-text with --headed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_interface_is_not_supported() {
        let cause = AppError::internal(
            "Accessibility bus error: org.freedesktop.DBus.Error.UnknownInterface: no such",
        );
        let error = editing_error("take typed text", &cause);
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
    }

    #[test]
    fn other_causes_keep_their_message() {
        let cause = AppError::internal("Accessibility bus error: something odd happened");
        let error = editing_error("take typed text", &cause);
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert!(error.message.contains("something odd happened"));
    }

    #[test]
    fn timeouts_stay_timeouts() {
        let cause = AppError::new(ErrorCode::Timeout, "The app did not answer in time");
        assert_eq!(
            editing_error("take typed text", &cause).code,
            ErrorCode::Timeout
        );
    }
}
