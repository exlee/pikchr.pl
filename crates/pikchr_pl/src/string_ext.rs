pub trait StringExt {
    fn trim_last_lines(self, lines: usize) -> Self;
    fn trim_last_chars(self, chars: usize) -> Self;
}

impl StringExt for String {
    fn trim_last_lines(self, lines: usize) -> Self {
        let mut reversed = Vec::new();
        for (idx, line) in self.lines().rev().enumerate() {
            if idx >= lines {
                break;
            }
            reversed.push(line);
        }
        reversed.into_iter().rev().collect::<Vec<_>>().join("\n")
    }

    fn trim_last_chars(self, chars: usize) -> Self {
        let mut reversed = Vec::new();
        for (idx, c) in self.chars().rev().enumerate() {
            if idx >= chars {
                break;
            }
            reversed.push(c)
        }
        let mut new_string = String::new();
        for c in reversed.into_iter().rev() {
            new_string.push(c)
        }
        new_string
    }
}
