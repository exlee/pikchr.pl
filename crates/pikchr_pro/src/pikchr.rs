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

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
};

use crate::{prolog::RenderError, types::*};

unsafe extern "C" {
    // char *pikchr(const char *zText, const char *zClass, unsigned int mFlags, int
    // *pWidth, int *pHeight);
    fn pikchr(
        zText: *const c_char,
        zClass: *const c_char,
        mFlags: c_int,
        pWidth: *mut c_int,
        pHeight: *mut c_int,
    ) -> *mut c_char;

    fn free(p: *mut c_void);
}

#[derive(Debug)]
pub struct PikchrResult {
    ptr:        *mut c_char,
    pub width:  i32,
    pub height: i32,
}

impl PikchrResult {
    pub fn as_str(&self) -> &str {
        unsafe {
            if self.ptr.is_null() {
                return "";
            }
            CStr::from_ptr(self.ptr).to_str().unwrap_or("")
        }
    }
    pub fn is_error(&self) -> bool {
        self.as_str().to_owned().contains("ERROR: ")
    }
    pub fn is_empty(&self) -> bool {
        self.as_str()
            .to_owned()
            .contains("<!-- empty pikchr diagram -->")
    }

    pub fn into_bytes(&self) -> Vec<u8> {
        self.into_string().as_bytes().to_owned()
    }

    pub fn into_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl From<PikchrResult> for SvgString {
    fn from(value: PikchrResult) -> Self {
        SvgString::new(value.into_string())
    }
}

impl Drop for PikchrResult {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                free(self.ptr as *mut c_void);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PikchrCode(String);
impl PikchrCode {
    pub fn new(input: impl AsRef<str>) -> Self {
        let s = String::from(input.as_ref());
        Self(s)
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}
impl From<String> for PikchrCode {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub fn render_pikchr(input: PikchrCode) -> Result<SvgString, RenderError> {
    let result = render(input.0.as_str(), None, 0).map_err(RenderError::PikchrError)?;
    Ok(SvgString::from(result))

}
pub fn render(text: &str, class_name: Option<&str>, flags: i32) -> Result<PikchrResult, String> {
    let c_text = CString::new(text).map_err(|e| e.to_string())?;
    let c_class = class_name
        .and_then(|s| CString::new(s).ok())
        .unwrap_or_else(|| CString::new("").unwrap());

    let mut width: c_int = 0;
    let mut height: c_int = 0;

    unsafe {
        let res_ptr = pikchr(
            c_text.as_ptr(),
            c_class.as_ptr(),
            flags,
            &mut width,
            &mut height,
        );

        if res_ptr.is_null() {
            return Err("Pikchr returned null pointer".to_string());
        }

        Ok(PikchrResult {
            ptr: res_ptr,
            width,
            height,
        })
    }
}
