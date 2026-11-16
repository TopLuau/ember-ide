// Line Tools - text transformation commands.

fn init() {
    register_command("line-tools.sort", "Lines: Sort ascending");
    register_command("line-tools.dedupe", "Lines: Remove duplicates");
    register_command("line-tools.reverse", "Lines: Reverse order");
    register_command("line-tools.upper", "Text: UPPERCASE document");
    register_command("line-tools.lower", "Text: lowercase document");
    register_command("line-tools.timestamp", "Text: Append timestamp");
}

fn on_command(id) {
    let text = editor_text();

    if id == "line-tools.sort" {
        let lines = text.split("\n");
        lines.sort();
        set_editor_text(join(lines, "\n"));
    } else if id == "line-tools.dedupe" {
        let seen = #{};
        let out = [];
        for line in text.split("\n") {
            if !(line in seen) {
                seen[line] = true;
                out.push(line);
            }
        }
        set_editor_text(join(out, "\n"));
        notify(`removed ${text.split("\n").len() - out.len()} duplicate lines`);
    } else if id == "line-tools.reverse" {
        let lines = text.split("\n");
        lines.reverse();
        set_editor_text(join(lines, "\n"));
    } else if id == "line-tools.upper" {
        set_editor_text(text.to_upper());
    } else if id == "line-tools.lower" {
        set_editor_text(text.to_lower());
    } else if id == "line-tools.timestamp" {
        set_editor_text(text + "\n" + now());
    }
}
