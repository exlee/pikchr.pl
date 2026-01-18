pub use crate::pikchr::PikchrCode;
macro_rules! string_newtype {
    ($name: ident) => {
        pub struct $name(String);
        impl $name {
            pub fn new<T>(input: T) -> Self
            where
                T: AsRef<str>,
            {
                Self(String::from(input.as_ref()))
            }
            pub fn as_inner(&self) -> &String {
                &self.0
            }
            pub fn into_inner(self) -> String {
                self.0
            }
        }
    };
}
string_newtype!(SvgString);
string_newtype!(PrologCode);
