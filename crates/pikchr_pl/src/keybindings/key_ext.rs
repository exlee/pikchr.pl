
use iced::{
    keyboard::{Key, Modifiers},
    widget::text_editor::KeyPress,
};

pub trait KeypressLike {
    fn key(&self) -> Key;
    fn modifiers(&self) -> Modifiers;
}
impl KeypressLike for KeyPress {
    fn key(&self) -> Key {
        self.key.clone()
    }

    fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}
impl KeypressLike for iced::keyboard::Event {
    fn key(&self) -> Key {
        match self {
            iced::keyboard::Event::KeyPressed { key, .. } => key.clone(),
            iced::keyboard::Event::KeyReleased { key, .. } => key.clone(),
            iced::keyboard::Event::ModifiersChanged(..) => Key::Unidentified,
        }
    }

    fn modifiers(&self) -> Modifiers {
        match self {
            iced::keyboard::Event::KeyPressed { modifiers, .. } => *modifiers,
            iced::keyboard::Event::KeyReleased { modifiers, .. } => *modifiers,
            iced::keyboard::Event::ModifiersChanged(..) => Modifiers::NONE,
        }
    }
}
