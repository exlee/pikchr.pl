pub fn transform_heredoc(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut parts = input.split("===");

    if let Some(first) = parts.next() {
        output.push_str(first);
    }

    for (i, part) in parts.enumerate() {
        if i % 2 == 0 {
            output.push('"');
            output.extend(part.escape_debug());
            output.push('"');
        } else {
            output.push_str(part);
        }
    }

    output
}
