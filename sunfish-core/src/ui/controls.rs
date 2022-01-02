use iced_baseview::{Align, Column, Command, Element, Length};
use iced_native::clipboard;
use iced_native::program::Program;
use iced_wgpu::Renderer;

pub struct Controls {}

#[derive(Debug, Clone)]
pub enum Message {
    // TODO: Add messages here.
}

impl Controls {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Controls {
        Controls {}
    }
}

impl Program for Controls {
    type Renderer = Renderer;
    type Message = Message;
    type Clipboard = clipboard::Null;

    fn update(&mut self, _message: Message, _clipboard: &mut clipboard::Null) -> Command<Message> {
        Command::none()
    }

    fn view(&mut self) -> Element<Message> {
        Column::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .align_items(Align::End)
            .into()
    }
}
