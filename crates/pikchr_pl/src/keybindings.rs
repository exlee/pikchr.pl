// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.

use iced::{
    event::{self, Event},
    keyboard::{self, Key},
    widget::text_editor::{Action, Binding, Edit, KeyPress, Motion},
};

use crate::Message;

mod key_ext;
macro_rules! key_dispatch {
    ($key:expr, {
        $(named: { $($name:ident => $n_msg:expr),* $(,)? },)?
        $(literals: { $($lit:literal => $l_msg:expr),* $(,)? })?
    }) => {
        match $key {
            $(
                $( Key::Named(keyboard::key::Named::$name) => Some($n_msg), )*
            )?
            $(
                $( Key::Character(c) if c.as_str() == $lit => Some($l_msg), )*
            )?

            _ => None,
        }
    };
}

pub fn handle_action(keypress: KeyPress) -> Option<Binding<Message>> {
    let custom_bind = |m: Message| Binding::Custom(m);

    if global_binding(keypress.clone()).is_some() {
        return None;
    }

    if keypress.modifiers.control() {
        return map_emacs_binding(keypress.key).map(custom_bind);
    }
    if keypress.modifiers.alt() && keypress.modifiers.shift() {
        return map_emacs_alt_shift_binding(keypress.key).map(custom_bind);
    }
    if keypress.modifiers.alt() {
        return map_emacs_alt_binding(keypress.key).map(custom_bind);
    }

    Binding::from_key_press(keypress)
}

fn global_binding(keypress: impl key_ext::KeypressLike) -> Option<Message> {
    if keypress.modifiers().command() && keypress.modifiers().shift() {
        key_dispatch!(keypress.key(), {
            named: {},
            literals: {
                "z" => Message::Redo,
            }
        })
    } else if keypress.modifiers().command() {
        key_dispatch!(keypress.key(), {
            named: {},
            literals: {
                "z" => Message::Undo,
            }
        })
    } else {
        key_dispatch!(keypress.key(), {
            named: {
                F2 => Message::ToggleDebugOverlay
            },
            literals: {}
        })
    }
}

pub fn listen() -> iced::Subscription<Message> {
    event::listen_with(|event, status, _window_id| {
        if status == event::Status::Captured {
            return None;
        }
        if let Event::Keyboard(keyboard_event) = event {
            match keyboard_event.clone() {
                keyboard::Event::ModifiersChanged(modifiers) => {
                    Some(Message::ModifiersChanged(modifiers))
                },
                keyboard::Event::KeyPressed { .. } => {
                    // TODO: Remember which key - needed for alt checks
                    //if key == Key::Named(keyboard::key::Named::Alt) {}
                    //if key == Key::Named(keyboard::key::Named::AltGraph) {}
                    global_binding(keyboard_event)
                },
                _ => None,
            }
        } else {
            None
        }
    })
}
fn map_emacs_alt_shift_binding(key: Key) -> Option<Message> {
    key_dispatch!(key, {
        named: {
            ArrowRight => select_word_right(),
            ArrowLeft => select_word_left()
        },
    })
}
fn map_emacs_alt_binding(key: Key) -> Option<Message> {
    key_dispatch!(key, {
        named: {
            Backspace => delete_word()
        },
        literals: {
            "f" => Message::PerformAction(Action::Move(Motion::WordRight)),
            "b" => Message::PerformAction(Action::Move(Motion::WordLeft)),
        }
    })
}

fn delete_word() -> Message {
    Message::PerformActions(
        true,
        vec![
            Action::Move(Motion::WordLeft),
            Action::Select(Motion::WordRight),
            Action::Edit(Edit::Delete),
        ],
    )
}

fn select_word_left() -> Message {
    Message::PerformAction(Action::Select(Motion::WordLeft))
}
fn select_word_right() -> Message {
    Message::PerformAction(Action::Select(Motion::WordRight))
}
fn map_emacs_binding(key: Key) -> Option<Message> {
    key_dispatch!(key.clone(), {
        named: {
            Backspace => Message::PerformActions(
                true,
                vec![Action::SelectLine, Action::Edit(Edit::Delete)],
            )
        },
        literals: {
            "n" => Message::PerformAction(Action::Move(Motion::Down)),
            "p" => Message::PerformAction(Action::Move(Motion::Up)),
            "f" => Message::PerformAction(Action::Move(Motion::Right)),
            "b" => Message::PerformAction(Action::Move(Motion::Left)),
            "a" => Message::PerformAction(Action::Move(Motion::Home)),
            "e" => Message::PerformAction(Action::Move(Motion::End)),
            "o" => Message::PerformActions(true,
                vec![
                    Action::Move(Motion::Home),
                    Action::Edit(Edit::Insert('\n')),
                    Action::Move(Motion::Left),
                ],
            ),
            "k" => Message::PerformActions( true,
                vec![Action::Select(Motion::End), Action::Edit(Edit::Delete)],
            ),
        }
    })
}
