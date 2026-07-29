use super::ModalAction;

#[derive(Clone, Debug)]
pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub action: ModalAction,
    pub destructive: bool,
    pub error: Option<String>,
}

impl ConfirmModal {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        action: ModalAction,
        destructive: bool,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            action,
            destructive,
            error: None,
        }
    }
}
