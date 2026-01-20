// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.


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
