// Trim Whitespace

fn init() {
    register_command("trim-whitespace.run", "Trim Trailing Whitespace");
}

fn on_command(id) {
    let out = [];
    let changed = 0;
    for line in editor_text().split("\n") {
        let t = rtrim(line);
        if t.len() != line.len() { changed += 1; }
        out.push(t);
    }
    set_editor_text(join(out, "\n"));
    notify(`trimmed ${changed} line(s)`);
}
